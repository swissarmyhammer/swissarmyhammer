//! Auto-install the bundled `kanban` CLI onto the user's `PATH` at launch.
//!
//! `Kanban.app` ships the standalone `kanban` CLI as a Tauri sidecar at
//! `Contents/MacOS/kanban` (see the sidecar-bundling task). This module makes
//! that binary reachable from a terminal by maintaining a `kanban` symlink in
//! a directory that is both user-writable and on the default `PATH`.
//!
//! Design goals:
//!
//! * **Silent.** There is no menu item and no click. [`run`] is called once
//!   per launch on a background thread and never blocks startup.
//! * **Idempotent + self-healing.** Re-running is a no-op when the link is
//!   already correct, and a stale link left behind by a moved or replaced
//!   `Kanban.app` is repaired in place.
//! * **Non-destructive.** A pre-existing real (non-symlink) `kanban` file —
//!   some unrelated tool of the same name — is never overwritten.
//! * **Homebrew-aware.** When the Homebrew cask already linked the CLI,
//!   [`already_installed`] short-circuits the whole flow so the user is never
//!   prompted.
//!
//! The pure functions ([`resolve_bundled_cli`], [`already_installed`],
//! [`install_cli_symlink`]) are filesystem-only and fully unit-tested. The
//! privilege-escalation path is isolated behind [`pick_target_dir`] and is
//! deliberately not unit-tested — see the comment on that function.

use std::io;
use std::path::{Path, PathBuf};

/// The filename of the bundled CLI and of the symlink created on `PATH`.
const CLI_NAME: &str = "kanban";

/// Fallback `PATH` directory used when no Homebrew prefix is writable. It is
/// on the default `PATH` of every macOS install but is typically root-owned,
/// so writing here needs privilege escalation.
const SYSTEM_BIN: &str = "/usr/local/bin";

/// Result of an [`install_cli_symlink`] attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// No `kanban` existed in the target dir; a fresh symlink was created.
    Created,
    /// A `kanban` symlink already pointed at the bundled CLI; nothing changed.
    AlreadyCurrent,
    /// A `kanban` symlink pointed into a *different* Kanban bundle (a moved or
    /// replaced `Kanban.app`); it was repaired to point at the current bundle.
    Repaired,
    /// A `kanban` entry exists that is not a symlink into a Kanban bundle —
    /// either a real file or a symlink to an unrelated target. It was left
    /// untouched to avoid clobbering an unrelated tool.
    Skipped,
}

/// A chosen install location together with whether writing to it requires
/// administrator privileges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDir {
    /// The directory in which to create the `kanban` symlink.
    pub path: PathBuf,
    /// True when `path` is not writable by the current user and the symlink
    /// must therefore be created via an `osascript` privilege escalation.
    pub needs_escalation: bool,
}

/// Resolve the bundled `kanban` CLI that ships next to the running app.
///
/// Tauri copies the sidecar into `Contents/MacOS/` alongside the main
/// `kanban-app` executable, so the CLI is simply the `kanban` sibling of
/// `current_exe`.
///
/// # Parameters
/// * `current_exe` — the path of the running app executable, as returned by
///   [`std::env::current_exe`].
///
/// # Returns
/// `Some(path)` to the sibling `kanban` when it exists on disk, `None` when no
/// such sibling is present (e.g. a `cargo run` build with no staged sidecar).
pub fn resolve_bundled_cli(current_exe: &Path) -> Option<PathBuf> {
    let candidate = current_exe.parent()?.join(CLI_NAME);
    candidate.exists().then_some(candidate)
}

/// Whether the trailing path components `Kanban.app/Contents/MacOS/kanban`
/// appear in `path` — i.e. `path` points at a `kanban` binary inside any
/// Kanban app bundle, regardless of where that bundle lives on disk.
fn is_inside_kanban_bundle(path: &Path) -> bool {
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    components
        .windows(4)
        .any(|w| w[0] == "Kanban.app" && w[1] == "Contents" && w[2] == "MacOS" && w[3] == CLI_NAME)
}

/// Whether `kanban` already resolves in `target_dir` to a symlink that points
/// into *some* `Kanban.app` bundle.
///
/// This covers the Homebrew-cask case: when `brew` installed the cask it
/// already created a `kanban` link into `Caskroom/.../Kanban.app`. In that
/// situation the app must do nothing — the CLI is reachable and re-linking it
/// would only fight the package manager.
///
/// # Parameters
/// * `target_dir` — the `PATH` directory to inspect.
/// * `bundled` — the CLI shipped with the *running* app. Unused for the
///   bundle-shape check but kept in the signature so callers always pass the
///   same pair to [`already_installed`] and [`install_cli_symlink`].
///
/// # Returns
/// `true` when a `kanban` symlink into any Kanban bundle is present, `false`
/// when the entry is missing, is a real file, or links somewhere unrelated.
pub fn already_installed(target_dir: &Path, bundled: &Path) -> bool {
    let _ = bundled;
    let link = target_dir.join(CLI_NAME);
    match std::fs::read_link(&link) {
        Ok(dest) => is_inside_kanban_bundle(&dest),
        // Not a symlink, or does not exist — nothing is installed.
        Err(_) => false,
    }
}

/// Create or repair the `kanban` symlink in `target_dir` so it points at
/// `bundled`.
///
/// The operation is idempotent and non-destructive:
///
/// * No `kanban` entry → create the symlink ([`InstallOutcome::Created`]).
/// * A symlink already pointing at `bundled` → no-op
///   ([`InstallOutcome::AlreadyCurrent`]).
/// * A symlink pointing into a *different* Kanban bundle → atomically replaced
///   to point at `bundled` ([`InstallOutcome::Repaired`]).
/// * Anything else — a real file, or a symlink to a non-Kanban target → left
///   untouched ([`InstallOutcome::Skipped`]) so an unrelated `kanban` tool is
///   never clobbered.
///
/// # Parameters
/// * `bundled` — absolute path to the bundled CLI to link to.
/// * `target_dir` — the writable `PATH` directory to place the link in.
///
/// # Errors
/// Returns any [`io::Error`] from reading the existing entry's metadata or
/// from creating/replacing the symlink.
pub fn install_cli_symlink(bundled: &Path, target_dir: &Path) -> io::Result<InstallOutcome> {
    let link = target_dir.join(CLI_NAME);

    match std::fs::symlink_metadata(&link) {
        // Nothing there yet — create a fresh link.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            create_symlink(bundled, &link)?;
            Ok(InstallOutcome::Created)
        }
        Err(e) => Err(e),
        Ok(meta) => {
            if !meta.file_type().is_symlink() {
                // A real file — an unrelated tool. Never overwrite it.
                return Ok(InstallOutcome::Skipped);
            }
            let current = std::fs::read_link(&link)?;
            if current == bundled {
                Ok(InstallOutcome::AlreadyCurrent)
            } else if is_inside_kanban_bundle(&current) {
                // A stale link into a moved/replaced Kanban bundle — repair it.
                replace_symlink(bundled, &link)?;
                Ok(InstallOutcome::Repaired)
            } else {
                // A symlink to something unrelated — leave it alone.
                Ok(InstallOutcome::Skipped)
            }
        }
    }
}

/// Create a `kanban` -> `bundled` symlink at `link`.
#[cfg(unix)]
fn create_symlink(bundled: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(bundled, link)
}

/// Replace an existing symlink at `link` so it points at `bundled`.
///
/// Removing then re-creating is sufficient here: a stale `kanban` link is not
/// a hot path, and the brief window between the two calls only ever leaves the
/// CLI momentarily unresolvable, never broken.
#[cfg(unix)]
fn replace_symlink(bundled: &Path, link: &Path) -> io::Result<()> {
    std::fs::remove_file(link)?;
    std::os::unix::fs::symlink(bundled, link)
}

/// Non-Unix fallback. Symlink semantics differ on Windows and PATH
/// registration there is a separate, deferred task; this keeps the crate
/// compiling on all targets without claiming to do the install.
#[cfg(not(unix))]
fn create_symlink(_bundled: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "CLI symlink install is only implemented for Unix targets",
    ))
}

/// Non-Unix fallback — see [`create_symlink`].
#[cfg(not(unix))]
fn replace_symlink(_bundled: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "CLI symlink install is only implemented for Unix targets",
    ))
}

/// Whether the current process can create files in `dir`.
///
/// Probed empirically — ownership/mode arithmetic is unreliable across ACLs
/// and group membership — by attempting to create and immediately remove a
/// uniquely named temp file. A `dir` that does not exist is not writable.
fn is_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(".kanban-cli-install-probe-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// The Homebrew `bin` directory (`<brew --prefix>/bin`), if Homebrew is
/// installed and its prefix can be determined.
///
/// `brew --prefix` is the authoritative answer and covers both the
/// Apple-silicon (`/opt/homebrew`) and Intel (`/usr/local`) layouts without
/// hardcoding either.
fn homebrew_bin() -> Option<PathBuf> {
    let output = std::process::Command::new("brew")
        .arg("--prefix")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let prefix = String::from_utf8(output.stdout).ok()?;
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return None;
    }
    Some(PathBuf::from(prefix).join("bin"))
}

/// Choose where to install the `kanban` symlink.
///
/// Preference order:
///
/// 1. The Homebrew `bin` directory when it exists and is user-writable — the
///    common case on a developer machine, and always on `PATH`. No escalation.
/// 2. Otherwise [`SYSTEM_BIN`] (`/usr/local/bin`), which is on the default
///    `PATH` everywhere but is usually root-owned, so `needs_escalation` is
///    set whenever it is not directly writable.
///
/// # Privilege escalation is intentionally not unit-tested.
///
/// The `needs_escalation` branch causes [`run`] to invoke `osascript` with
/// `with administrator privileges`, which opens a system password dialog.
/// That cannot be exercised in an automated test, and faking it would test the
/// fake rather than the behaviour. Coverage stops at this function: it
/// produces a plain [`TargetDir`] value, and every consumer ([`run`]) branches
/// only on its boolean. The pure symlink functions above are fully tested.
pub fn pick_target_dir() -> TargetDir {
    if let Some(brew_bin) = homebrew_bin() {
        if is_writable(&brew_bin) {
            return TargetDir {
                path: brew_bin,
                needs_escalation: false,
            };
        }
    }
    let system_bin = PathBuf::from(SYSTEM_BIN);
    let needs_escalation = !is_writable(&system_bin);
    TargetDir {
        path: system_bin,
        needs_escalation,
    }
}

/// Filename of the marker recording that a privileged install was already
/// attempted, so a user who declines the password prompt is not nagged on
/// every subsequent launch.
const ESCALATION_MARKER: &str = ".cli-install-attempted";

/// Path of the privileged-install marker file inside the app's
/// Application Support directory, if that directory can be determined.
fn escalation_marker_path() -> Option<PathBuf> {
    let support = dirs::data_dir()?.join("com.swissarmyhammer.kanban");
    Some(support.join(ESCALATION_MARKER))
}

/// Install the `kanban` symlink into a root-owned directory via a single
/// `osascript` privilege escalation, guarded by a one-shot marker file.
///
/// The first launch that needs escalation shows the macOS password dialog
/// exactly once. Whether the user accepts or declines, the marker is written
/// so later launches do not prompt again. A later launch can still self-heal
/// silently if a writable directory (e.g. a freshly installed Homebrew)
/// becomes available — [`run`] reaches this path only when escalation is the
/// *only* option.
fn install_with_escalation(bundled: &Path, target_dir: &Path) {
    let marker = escalation_marker_path();
    if let Some(marker) = &marker {
        if marker.exists() {
            tracing::debug!(
                "kanban CLI install: privileged attempt already made, not prompting again"
            );
            return;
        }
    }

    let link = target_dir.join(CLI_NAME);
    // `ln -sf` is idempotent and atomic enough for this one-shot path. The
    // inner double quotes are escaped for the AppleScript string literal.
    let shell = format!("ln -sf '{}' '{}'", bundled.display(), link.display());
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        shell.replace('\\', "\\\\").replace('"', "\\\"")
    );

    match std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
    {
        Ok(status) if status.success() => {
            tracing::info!(
                target_dir = %target_dir.display(),
                "kanban CLI installed via privilege escalation"
            );
        }
        Ok(status) => {
            // Most commonly: the user clicked Cancel on the password dialog.
            tracing::info!(
                %status,
                "kanban CLI privileged install was not completed"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "kanban CLI privileged install: osascript failed to run");
        }
    }

    // Record the attempt regardless of outcome so we never nag.
    if let Some(marker) = marker {
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&marker, b"") {
            tracing::warn!(error = %e, "kanban CLI install: failed to write attempt marker");
        }
    }
}

/// Ensure the bundled `kanban` CLI is reachable on `PATH`.
///
/// This is the module entry point. It is safe to call on every launch: it is
/// idempotent, self-healing, and never blocks — callers run it on a background
/// thread (see [`spawn`]).
///
/// The flow is: locate the bundled CLI next to the running app, pick a viable
/// `PATH` directory, short-circuit if a Kanban link is already present, then
/// either symlink directly (writable dir) or escalate once (root-owned dir).
/// Every outcome is logged via `tracing`; nothing is printed to stderr because
/// the GUI routes logs to the macOS unified log.
pub fn run() {
    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            tracing::warn!(error = %e, "kanban CLI install: cannot resolve current_exe");
            return;
        }
    };

    let Some(bundled) = resolve_bundled_cli(&current_exe) else {
        tracing::debug!("kanban CLI install: no bundled `kanban` next to the app; skipping");
        return;
    };

    let target = pick_target_dir();

    if already_installed(&target.path, &bundled) {
        tracing::debug!(
            target_dir = %target.path.display(),
            "kanban CLI install: already linked into a Kanban bundle; skipping"
        );
        return;
    }

    if target.needs_escalation {
        install_with_escalation(&bundled, &target.path);
        return;
    }

    match install_cli_symlink(&bundled, &target.path) {
        Ok(outcome) => tracing::info!(
            ?outcome,
            target_dir = %target.path.display(),
            bundled = %bundled.display(),
            "kanban CLI install completed"
        ),
        Err(e) => tracing::warn!(
            error = %e,
            target_dir = %target.path.display(),
            "kanban CLI install failed"
        ),
    }
}

/// Run [`run`] on a detached background thread so app startup is never
/// blocked by `brew --prefix`, filesystem probes, or an `osascript` dialog.
pub fn spawn() {
    std::thread::spawn(run);
}
