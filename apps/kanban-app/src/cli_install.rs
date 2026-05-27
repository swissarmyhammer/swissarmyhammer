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
//! * **Homebrew-aware.** When the Homebrew cask already linked the CLI from
//!   `/Applications/Kanban.app`, an app launched from `/Applications` finds
//!   that link already pointing at its own CLI, so [`already_installed`]
//!   short-circuits the whole flow and the user is never prompted.
//!
//! Self-install is gated **solely on the `kanban` symlink** — there is no
//! remembered-attempt state. Each launch, [`run`] checks
//! [`already_installed`]: a symlink pointing exactly at the running app's CLI
//! is a silent no-op, while a missing link — or one pointing anywhere else —
//! triggers a fresh install/repair attempt (including the escalation/password
//! path on macOS). So if the link is later deleted, the next launch re-creates
//! it.
//!
//! The pure functions ([`resolve_bundled_cli`], [`already_installed`],
//! [`install_cli_symlink`], [`build_install_applescript`]) are
//! computation-only and fully unit-tested. The privilege-escalation path —
//! constructing and running an `NSAppleScript` — is isolated behind
//! [`pick_target_dir`] and is deliberately not unit-tested; see the comment on
//! that function.

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
    /// must therefore be created via an in-process AppleScript privilege
    /// escalation (macOS only).
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

/// Whether `kanban` in `target_dir` is *our* symlink — a symlink pointing
/// exactly at `bundled`, the CLI of the currently running app.
///
/// The question this answers is "is this our install", not "is there a
/// `kanban`". Only an exact match means the link already does what [`run`]
/// would do, so the launch-time install can be skipped. Every other state —
/// a missing entry, a real (non-symlink) file, a symlink to an unrelated
/// target, or a symlink into a *different* `Kanban.app` bundle — returns
/// `false`, so [`run`] proceeds to install or repair the link.
///
/// This still short-circuits the Homebrew-cask case correctly: a cask links
/// `<brew bin>/kanban -> /Applications/Kanban.app/Contents/MacOS/kanban`, and
/// when the app is launched from `/Applications` its `bundled` path *is* that
/// target. The exact-match check then returns `true`, so the app does not
/// fight the package manager.
///
/// # Parameters
/// * `target_dir` — the `PATH` directory to inspect.
/// * `bundled` — the CLI shipped with the *running* app; the symlink target
///   that counts as already installed.
///
/// # Returns
/// `true` only when `target_dir/kanban` is a symlink whose target equals
/// `bundled` exactly; `false` for every other state.
pub fn already_installed(target_dir: &Path, bundled: &Path) -> bool {
    let link = target_dir.join(CLI_NAME);
    match std::fs::read_link(&link) {
        // A symlink — installed only when it points at the running app's CLI.
        Ok(dest) => dest == bundled,
        // Not a symlink (a real file), or does not exist — not our install.
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
/// The `needs_escalation` branch causes [`run`] (on macOS) to run an
/// AppleScript `with administrator privileges`, which opens a system password
/// dialog. That cannot be exercised in an automated test, and faking it would
/// test the fake rather than the behaviour. Coverage stops at this function:
/// it produces a plain [`TargetDir`] value, and every consumer ([`run`])
/// branches only on its boolean. The pure functions above — including
/// [`build_install_applescript`], which renders the exact AppleScript source —
/// are fully tested.
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

/// Escape a string for embedding inside an AppleScript double-quoted literal.
///
/// AppleScript string literals treat `\` and `"` specially, so each must be
/// backslash-escaped. Order matters: backslashes are doubled first so the
/// backslashes introduced when escaping quotes are not doubled again.
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Render the AppleScript source that installs the `kanban` CLI symlink.
///
/// The script has two parts:
///
/// 1. An explanatory `display dialog` that names the `kanban` command-line
///    tool and offers an `Install` / `Not Now` choice. `Install` is the
///    default button and `Not Now` is the *cancel* button — clicking it makes
///    AppleScript raise the standard user-cancelled error (-128), which aborts
///    the script before the privileged step ever runs. That is exactly the
///    behaviour wanted: choosing `Not Now` is treated like a declined prompt.
/// 2. A `do shell script "ln -sf …" with administrator privileges`, reached
///    only when the user chose `Install`. Running this from inside the
///    `Kanban.app` process makes macOS attribute the password dialog to
///    "Kanban" rather than to a spawned `osascript` binary.
///
/// This function is pure and platform-neutral — it builds and returns a
/// `String`, touching no macOS types — so it compiles and is unit-tested on
/// any host.
///
/// # Parameters
/// * `bundled` — absolute path to the bundled CLI to link to.
/// * `link` — absolute path of the `kanban` symlink to create on `PATH`.
pub fn build_install_applescript(bundled: &Path, link: &Path) -> String {
    // The shell command run with administrator privileges. `ln -sf` is
    // idempotent and atomic enough for this install step. The whole command
    // becomes an AppleScript string literal, so it is escaped as one.
    let shell = format!("ln -sf '{}' '{}'", bundled.display(), link.display());
    let shell_literal = applescript_escape(&shell);

    // The explanatory prose shown before the password prompt. It is a separate
    // AppleScript string literal and so is escaped independently.
    let explanation = applescript_escape(
        "Kanban can install the \"kanban\" command-line tool so you can use it \
         from the Terminal.\n\nThis needs your administrator password to add \
         the command to your PATH.",
    );

    format!(
        "display dialog \"{explanation}\" \
         with title \"Install kanban command-line tool\" \
         buttons {{\"Not Now\", \"Install\"}} \
         default button \"Install\" cancel button \"Not Now\" with icon note\n\
         do shell script \"{shell_literal}\" with administrator privileges"
    )
}

/// Install the `kanban` symlink into a root-owned directory by running an
/// AppleScript `with administrator privileges` in-process.
///
/// [`run`] calls this only when [`already_installed`] is false and escalation
/// is the *only* option, so every call corresponds to a `kanban` link that is
/// missing or does not point at this app's CLI. It builds the AppleScript via
/// [`build_install_applescript`] and runs it unconditionally: such a launch
/// shows the explanatory dialog and, if the user chooses `Install`, the macOS
/// password dialog. There is no remembered-attempt state — if the user
/// declines, the next launch (still not finding our link) offers again; once
/// the link points at this app's CLI, [`already_installed`] short-circuits
/// before this function is ever reached.
///
/// # In-process AppleScript, not a subprocess
///
/// The script is executed via `NSAppleScript` inside the running `Kanban.app`
/// process rather than by spawning the `osascript` binary. macOS attributes
/// the `with administrator privileges` request to the *process* that makes it,
/// so running in-process makes the auth dialog read "Kanban wants to make
/// changes" instead of the opaque "osascript wants to make changes".
///
/// # Not unit-tested
///
/// Constructing an `NSAppleScript` and calling `executeAndReturnError` opens
/// system dialogs and triggers a privilege prompt — GUI/privilege side effects
/// that cannot be exercised in an automated test. The testable part is fully
/// isolated in [`build_install_applescript`]; this function only wraps that
/// pure source in the unavoidable `NSAppleScript` call, mirroring the untested
/// escalation boundary documented on [`pick_target_dir`].
#[cfg(target_os = "macos")]
fn install_with_escalation(bundled: &Path, target_dir: &Path) {
    use objc2::AnyThread;
    use objc2_foundation::{NSAppleScript, NSString};

    let link = target_dir.join(CLI_NAME);
    let source = build_install_applescript(bundled, &link);

    // Build and run the AppleScript in-process. `NSAppleScript::initWithSource`
    // returns `None` only when the source string is unrepresentable, which
    // cannot happen for the ASCII-and-path script built above.
    let ns_source = NSString::from_str(&source);
    match NSAppleScript::initWithSource(NSAppleScript::alloc(), &ns_source) {
        Some(script) => {
            let mut error: Option<_> = None;
            // SAFETY: `script` is a freshly created `NSAppleScript`; passing a
            // valid out-pointer for the error dictionary is the documented
            // calling convention for `executeAndReturnError:`.
            let _descriptor = unsafe { script.executeAndReturnError(Some(&mut error)) };
            match error {
                None => tracing::info!(
                    target_dir = %target_dir.display(),
                    "kanban CLI installed via privilege escalation"
                ),
                Some(_) => {
                    // Most commonly: the user chose `Not Now` (AppleScript
                    // error -128) or cancelled the password dialog.
                    tracing::info!("kanban CLI privileged install was not completed");
                }
            }
        }
        None => {
            tracing::warn!("kanban CLI privileged install: AppleScript source could not be built");
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
/// either symlink directly (writable dir) or escalate (root-owned dir).
/// Whether to act is gated solely on the symlink — a launch that still finds
/// no link attempts the install again.
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
            "kanban CLI install: `kanban` already links at this app's CLI; skipping"
        );
        return;
    }

    if target.needs_escalation {
        // Privileged CLI install is implemented only on macOS, where the
        // `NSAppleScript` `with administrator privileges` flow exists. On
        // other platforms there is no AppleScript to run and no prompt to
        // show, so this branch honestly does nothing — it does not fake an
        // install.
        #[cfg(target_os = "macos")]
        install_with_escalation(&bundled, &target.path);
        #[cfg(not(target_os = "macos"))]
        tracing::debug!(
            target_dir = %target.path.display(),
            "kanban CLI install: target dir needs privilege escalation, \
             which is only supported on macOS; skipping"
        );
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
/// blocked by `brew --prefix`, filesystem probes, or a privileged-install
/// AppleScript dialog.
pub fn spawn() {
    std::thread::spawn(run);
}
