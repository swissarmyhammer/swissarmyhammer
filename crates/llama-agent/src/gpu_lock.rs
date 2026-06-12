//! Machine-wide cross-process advisory lock that serializes GPU generation.
//!
//! Many `sah serve` processes run on one machine (one per interactive `claude`
//! session and per `/finish` subagent). They already SHARE the model weights
//! via mmap + a Metal shared buffer, so there is one resident copy of the
//! weights — but the GPU itself is singular. N processes submitting decode work
//! at once merely timeshare the one device, with no throughput gain and added
//! scheduler/context-switch contention.
//!
//! [`GpuLock`] is the cross-process generalization of the in-process queue gate
//! (the local llama queue already runs one generation turn at a time within a
//! process). It is an OS advisory file lock (`flock(2)` via [`fs2`]) on a
//! well-known, model-keyed file under the system temp directory. Because the
//! lock is keyed on a per-machine path shared by every serve process, only one
//! process holds it at a time — the others BLOCK in [`fs2::FileExt::lock_exclusive`]
//! until it is released, rather than erroring.
//!
//! The lock is **crash-safe**: the kernel releases an `flock` when the holding
//! process exits for any reason (clean exit, panic, SIGKILL), so a serve that
//! dies mid-generation cannot wedge the machine — there is no stale-lock
//! recovery to write. Releasing on a clean drop is handled by
//! [`GpuLockGuard::drop`]; crash release is the kernel's job.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, info};

/// Failure to acquire the machine-wide GPU lock.
///
/// Each variant names the step that failed and carries the full lock path, so
/// a caller (or a log line built from `Display`) can tell *which* filesystem
/// operation on *which* lock file went wrong — not just the bare OS error.
#[derive(Debug, thiserror::Error)]
pub enum GpuLockError {
    /// Creating the lock file's parent directory failed.
    #[error("failed to create parent directory for GPU lock file {path}: {source}")]
    CreateDir {
        /// The lock file whose parent directory could not be created.
        path: PathBuf,
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// Opening (or creating) the lock file itself failed.
    #[error("failed to open GPU lock file {path}: {source}")]
    Open {
        /// The lock file that could not be opened.
        path: PathBuf,
        /// The underlying filesystem error.
        source: io::Error,
    },
    /// The `flock(LOCK_EX)` syscall on the opened lock file failed.
    #[error("failed to take exclusive flock on GPU lock file {path}: {source}")]
    Lock {
        /// The lock file the flock was attempted on.
        path: PathBuf,
        /// The underlying flock error.
        source: io::Error,
    },
}

/// Prefix for the per-machine GPU lock file living in the system temp dir.
///
/// The full filename embeds the model-source identity (see
/// [`gpu_lock_path`]) so two different models could in principle take turns on
/// distinct locks; in the common one-GPU/one-model deployment there is a single
/// lock file. The temp dir (not a git-repo-relative `.sah` dir) is deliberate:
/// the lock must be shared by every serve process on the machine regardless of
/// which repository it was launched from — one GPU means one lock.
const GPU_LOCK_PREFIX: &str = "sah-gpu-lock-";

/// Derive the machine-wide GPU lock file path for a given model identity.
///
/// `model_key` is the model-source hash (`ModelConfig::compute_model_hash`),
/// reused so the lock name is data-driven from the loaded model rather than a
/// second hardcoded literal. The path is rooted at [`std::env::temp_dir`], the
/// same machine-wide coordination location the leader-election crate uses for
/// its cross-process flock.
pub fn gpu_lock_path(model_key: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{GPU_LOCK_PREFIX}{model_key}.lock"))
}

/// Held while a process owns the machine-wide GPU. The `flock` is released when
/// this guard drops (clean path) and by the kernel if the process dies first
/// (crash path).
#[derive(Debug)]
pub struct GpuLockGuard {
    file: File,
    /// Retained so the release event can name the lock being released.
    lock_path: PathBuf,
}

impl Drop for GpuLockGuard {
    fn drop(&mut self) {
        // Best-effort explicit unlock on the clean path. If this fails (or the
        // process never reaches here), the kernel still releases the flock when
        // the fd is closed / the process exits, so there is no stale lock.
        let _ = FileExt::unlock(&self.file);
        debug!("released GPU lock at {}", self.lock_path.display());
    }
}

/// Cross-process advisory lock serializing GPU generation across all serve
/// processes on the machine.
///
/// Cloning is cheap — the lock identity is just the file path; each
/// [`GpuLock::acquire_blocking`] opens its own file descriptor and takes the
/// `flock` afresh.
#[derive(Debug, Clone)]
pub struct GpuLock {
    lock_path: PathBuf,
}

impl GpuLock {
    /// Create a lock handle for the given model identity. Does not touch the
    /// filesystem until [`acquire_blocking`](Self::acquire_blocking) is called.
    pub fn for_model(model_key: &str) -> Self {
        Self {
            lock_path: gpu_lock_path(model_key),
        }
    }

    /// Create a lock handle for an explicit path. Used by tests with a temp
    /// path so they never touch the real machine-wide lock.
    pub fn at_path(lock_path: impl Into<PathBuf>) -> Self {
        Self {
            lock_path: lock_path.into(),
        }
    }

    /// The path of the underlying lock file.
    pub fn path(&self) -> &std::path::Path {
        &self.lock_path
    }

    /// Acquire the exclusive cross-process lock, BLOCKING until it is free.
    ///
    /// This is a synchronous, blocking call (`flock(LOCK_EX)`): when another
    /// process holds the lock it parks until that process releases it (or dies,
    /// at which point the kernel releases it). It MUST therefore be run off the
    /// async executor — callers in async contexts wrap it in
    /// `tokio::task::spawn_blocking` so a held lock never stalls the runtime.
    ///
    /// # Errors
    ///
    /// Returns a [`GpuLockError`] naming the failing step and the lock path:
    /// - [`GpuLockError::CreateDir`] — the lock file's parent directory could
    ///   not be created.
    /// - [`GpuLockError::Open`] — the lock file could not be opened/created.
    /// - [`GpuLockError::Lock`] — the `flock(LOCK_EX)` syscall failed.
    pub fn acquire_blocking(&self) -> Result<GpuLockGuard, GpuLockError> {
        if let Some(parent) = self.lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| GpuLockError::CreateDir {
                path: self.lock_path.clone(),
                source,
            })?;
        }
        // Do not truncate: the file is a pure lock token, its contents are
        // irrelevant, and another process may hold the lock concurrently.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)
            .map_err(|source| GpuLockError::Open {
                path: self.lock_path.clone(),
                source,
            })?;
        info!("waiting for GPU lock at {}", self.lock_path.display());
        let wait_started = Instant::now();
        FileExt::lock_exclusive(&file).map_err(|source| GpuLockError::Lock {
            path: self.lock_path.clone(),
            source,
        })?;
        info!(
            "acquired GPU lock at {} (waited {:?})",
            self.lock_path.display(),
            wait_started.elapsed()
        );
        Ok(GpuLockGuard {
            file,
            lock_path: self.lock_path.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// The lock path is derived from the model key, lives under the system temp
    /// dir, and is stable for the same key (machine-wide identity).
    #[test]
    fn lock_path_is_model_keyed_and_under_temp_dir() {
        let p = gpu_lock_path("abc123");
        assert!(p.starts_with(std::env::temp_dir()));
        assert!(p.file_name().unwrap().to_string_lossy().contains("abc123"));
        // Stable for the same key.
        assert_eq!(p, gpu_lock_path("abc123"));
        // Distinct per model identity.
        assert_ne!(gpu_lock_path("abc123"), gpu_lock_path("def456"));
    }

    /// Acquiring the lock emits "waiting" and "acquired" tracing events that
    /// carry the full lock file path (never truncated), and dropping the guard
    /// emits a release event — so cross-process GPU serialization is visible
    /// in the tracing log.
    ///
    /// Uses a scoped (thread-local) subscriber with the shared in-memory
    /// [`CaptureWriter`](swissarmyhammer_common::test_utils::CaptureWriter)
    /// rather than `#[tracing_test::traced_test]`: other tests in this binary
    /// (the chat_template suite) install the global dispatcher via
    /// `try_init()`, and `traced_test`'s second global registration panics.
    /// Every event asserted here is emitted on the test thread, so a scoped
    /// default captures all of them deterministically.
    ///
    /// Serialized with the other lock tests: they hit the SAME `info!`/`debug!`
    /// callsites from their own threads, and tracing-core caches callsite
    /// interest globally on first touch. With only a scoped dispatcher
    /// registered, a foreign thread's first touch evaluates interest against
    /// ITS thread default (`NoSubscriber`) and caches `Interest::never`,
    /// silently disabling the callsite for this test too.
    #[test]
    #[serial_test::serial(gpu_lock)]
    fn acquire_and_release_emit_tracing_events_with_lock_path() {
        use swissarmyhammer_common::test_utils::CaptureWriter;

        let capture = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(capture.clone())
            .finish();
        let _scope = tracing::subscriber::set_default(subscriber);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpu.lock");
        let path_str = path.display().to_string();
        let lock = GpuLock::at_path(path);

        let guard = lock.acquire_blocking().unwrap();
        // Waiting + acquired events, each carrying the full lock path.
        assert!(capture.contains(&format!("waiting for GPU lock at {path_str}")));
        assert!(capture.contains(&format!("acquired GPU lock at {path_str}")));
        // The acquired event reports how long the acquirer waited.
        assert!(capture.contains("waited"));

        // No release event until the guard drops.
        assert!(!capture.contains("released GPU lock"));
        drop(guard);
        assert!(capture.contains(&format!("released GPU lock at {path_str}")));
    }

    /// An acquisition failure must identify WHICH step failed and the lock
    /// path involved — not just the bare OS error ("Not a directory") that a
    /// caller cannot act on.
    #[test]
    fn acquire_error_names_failing_step_and_lock_path() {
        let dir = tempfile::tempdir().unwrap();
        // Make a component of the lock path's parent a regular FILE so
        // creating the parent directory must fail.
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"file").unwrap();
        let lock_path = blocker.join("sub").join("gpu.lock");
        let lock = GpuLock::at_path(lock_path.clone());

        let msg = lock.acquire_blocking().unwrap_err().to_string();
        assert!(
            msg.contains("create"),
            "error should name the failing step (directory creation): {msg}"
        );
        assert!(
            msg.contains(&lock_path.display().to_string()),
            "error should carry the full lock path: {msg}"
        );
    }

    /// A second acquirer of the SAME lock path must WAIT until the first guard
    /// releases (drops), not error.
    ///
    /// Serialized with the tracing-assertion test above — see its doc comment
    /// for the callsite-interest race this avoids.
    #[test]
    #[serial_test::serial(gpu_lock)]
    fn second_acquirer_waits_until_first_releases() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gpu.lock");
        let lock = GpuLock::at_path(path);

        let guard = lock.acquire_blocking().unwrap();

        // A background thread tries to take the same lock; it should block
        // until we drop `guard`.
        let lock2 = lock.clone();
        let acquired_at = std::sync::Arc::new(std::sync::Mutex::new(None::<Instant>));
        let acquired_at_bg = acquired_at.clone();
        let handle = std::thread::spawn(move || {
            let _g = lock2.acquire_blocking().unwrap();
            *acquired_at_bg.lock().unwrap() = Some(Instant::now());
        });

        // Deliberate hold window: long enough for the background acquirer to
        // park inside the blocking `flock` before we release.
        const HOLD_BEFORE_RELEASE: Duration = Duration::from_millis(200);
        std::thread::sleep(HOLD_BEFORE_RELEASE);
        let released_at = Instant::now();
        drop(guard);

        handle.join().unwrap();
        let acquired_at = acquired_at.lock().unwrap().unwrap();
        // The background acquirer should not have proceeded before we released.
        assert!(
            acquired_at >= released_at,
            "second acquirer proceeded before the first released the lock"
        );
    }

    /// The flock auto-releases when the holding PROCESS is killed mid-hold —
    /// no manual stale-lock recovery. A child process takes the lock, signals
    /// us, then we SIGKILL it; a fresh acquirer must then proceed.
    ///
    /// Deterministic and well under 10s: it uses a temp lock path and a
    /// short-lived child, never the real model.
    ///
    /// Serialized with the tracing-assertion test above — see its doc comment
    /// for the callsite-interest race this avoids.
    #[test]
    #[serial_test::serial(gpu_lock)]
    fn lock_auto_releases_when_holder_killed() {
        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("gpu.lock");
        // Sentinel the child creates AFTER it has taken the lock, so the parent
        // knows the lock is genuinely held before it kills the child.
        let ready_path = dir.path().join("child-holds-lock");

        let child_src = format!(
            r#"
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt as _;
fn main() {{
    let lock_path = {lock:?};
    let ready_path = {ready:?};
    let file = OpenOptions::new().read(true).write(true).create(true).open(&lock_path).unwrap();
    fs2::FileExt::lock_exclusive(&file).unwrap();
    std::fs::write(&ready_path, b"held").unwrap();
    // Hold the lock forever; the parent will SIGKILL us.
    loop {{ std::thread::sleep(std::time::Duration::from_secs(3600)); }}
}}
"#,
            lock = lock_path,
            ready = ready_path,
        );

        // Build a tiny child binary that takes the same flock via fs2 and holds
        // it. We compile it against this crate's already-built fs2 rlib so the
        // child does not need a network/registry fetch.
        let child = spawn_lock_holder_child(&child_src, dir.path());

        // Wait until the child reports it holds the lock. The deadline and
        // poll interval together bound the wait loop (~250 polls max).
        const CHILD_READY_DEADLINE: Duration = Duration::from_secs(5);
        const READY_POLL_INTERVAL: Duration = Duration::from_millis(20);
        let deadline = Instant::now() + CHILD_READY_DEADLINE;
        while !ready_path.exists() {
            assert!(Instant::now() < deadline, "child never acquired the lock");
            std::thread::sleep(READY_POLL_INTERVAL);
        }

        // Sanity: while the child holds it, WE cannot take it non-blocking.
        {
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_path)
                .unwrap();
            assert!(
                FileExt::try_lock_exclusive(&f).is_err(),
                "lock should be held by the child"
            );
        }

        // Kill the child mid-hold (SIGKILL — no chance to run Drop).
        unsafe {
            libc::kill(child.pid, libc::SIGKILL);
        }
        child.wait();

        // The kernel must have released the flock on process death: a fresh
        // acquirer now proceeds without any manual cleanup.
        let lock = GpuLock::at_path(lock_path);
        let start = Instant::now();
        let guard = lock.acquire_blocking().unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "acquiring after the holder was killed should be immediate"
        );
        drop(guard);
    }

    /// Handle to a spawned child process that holds the flock.
    struct LockHolderChild {
        pid: i32,
        handle: Option<std::process::Child>,
    }

    impl LockHolderChild {
        fn wait(mut self) {
            if let Some(mut h) = self.handle.take() {
                let _ = h.wait();
            }
        }
    }

    /// Compile and spawn a minimal child binary (source `src`) that links
    /// against this crate's resolved `fs2` rlib and runs in `work_dir`.
    ///
    /// We reuse the test build's dependency artifacts (`target/.../deps`) via
    /// `--extern fs2=<rlib>` so the child compiles offline and shares the exact
    /// `fs2` already in the tree — no new crate, no registry fetch.
    fn spawn_lock_holder_child(src: &str, work_dir: &std::path::Path) -> LockHolderChild {
        use std::process::Command;

        let src_path = work_dir.join("lock_holder.rs");
        std::fs::write(&src_path, src).unwrap();
        let bin_path = work_dir.join("lock_holder");

        let fs2_rlib = find_fs2_rlib();
        let deps_dir = fs2_rlib.parent().unwrap();

        let out = Command::new("rustc")
            .arg(&src_path)
            .arg("-o")
            .arg(&bin_path)
            .arg("--edition=2021")
            .arg("-L")
            .arg(deps_dir)
            .arg("--extern")
            .arg(format!("fs2={}", fs2_rlib.display()))
            .output()
            .expect("failed to invoke rustc for child");
        assert!(
            out.status.success(),
            "child compile failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        let handle = Command::new(&bin_path)
            .spawn()
            .expect("failed to spawn child lock holder");
        let pid = handle.id() as i32;
        LockHolderChild {
            pid,
            handle: Some(handle),
        }
    }

    /// Locate the `fs2` rlib produced for this test build under `target/.../deps`.
    fn find_fs2_rlib() -> PathBuf {
        // The test binary lives at target/<profile>/deps/<name>-<hash>; its
        // parent is the deps dir where the fs2 rlib also lives.
        let exe = std::env::current_exe().expect("current_exe");
        let deps_dir = exe.parent().expect("deps dir");
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in std::fs::read_dir(deps_dir).expect("read deps dir") {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("libfs2-") && name.ends_with(".rlib") {
                let mtime = entry.metadata().unwrap().modified().unwrap();
                if newest.as_ref().is_none_or(|(t, _)| mtime > *t) {
                    newest = Some((mtime, entry.path()));
                }
            }
        }
        newest.expect("fs2 rlib in deps dir").1
    }
}
