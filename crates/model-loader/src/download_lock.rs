//! Cross-process download coordination using file locks
//!
//! This module provides coordination for multiple processes attempting to download
//! the same model file simultaneously. It uses file-based locking to ensure only
//! one process downloads at a time while others wait.

use crate::error::ModelError;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info, warn};

/// Maximum time in seconds to wait for another process to complete a download (10 minutes)
const MAX_WAIT_SECS: u64 = 600;

/// How often in milliseconds to check if a download has completed
const POLL_INTERVAL_MS: u64 = 500;

/// How old in seconds a lock file can be before we consider it stale (5 minutes)
const STALE_LOCK_THRESHOLD_SECS: u64 = 300;

/// Short grace period before a lock whose owning process is confirmed dead is
/// treated as stale. Avoids racing a lock that was created moments ago.
const STALE_CHECK_MIN_AGE_SECS: u64 = 10;

/// Maximum time to wait for another process to complete a download
const MAX_WAIT_DURATION: Duration = Duration::from_secs(MAX_WAIT_SECS);

/// How often to check if a download has completed
const POLL_INTERVAL: Duration = Duration::from_millis(POLL_INTERVAL_MS);

/// How old a lock file can be before we consider it stale (crashed process)
const STALE_LOCK_THRESHOLD: Duration = Duration::from_secs(STALE_LOCK_THRESHOLD_SECS);

/// Coordinates downloads across multiple processes
pub struct DownloadCoordinator {
    lock_dir: PathBuf,
}

impl DownloadCoordinator {
    /// Create a new coordinator using the default cache directory
    pub fn new() -> Result<Self, ModelError> {
        let lock_dir = Self::default_lock_dir()?;
        fs::create_dir_all(&lock_dir)
            .map_err(|e| ModelError::Cache(format!("Failed to create lock directory: {}", e)))?;
        Ok(Self { lock_dir })
    }

    /// Get the default lock directory
    fn default_lock_dir() -> Result<PathBuf, ModelError> {
        // Use HuggingFace cache directory structure
        let cache_dir = std::env::var("HF_HOME")
            .or_else(|_| std::env::var("XDG_CACHE_HOME").map(|p| format!("{}/huggingface", p)))
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|h| h.join(".cache/huggingface").to_string_lossy().to_string())
                    .unwrap_or_else(|| "/tmp/huggingface".to_string())
            });

        Ok(PathBuf::from(cache_dir).join("download_locks"))
    }

    /// Generate a lock file path for a given repo and filename
    fn lock_path(&self, repo: &str, filename: &str) -> PathBuf {
        // Create a safe filename from repo/filename
        let safe_name = format!("{}_{}", repo.replace('/', "--"), filename).replace(
            |c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.',
            "_",
        );
        self.lock_dir.join(format!("{}.lock", safe_name))
    }

    /// Execute a download with cross-process coordination
    ///
    /// If another process is already downloading, this will wait for it to complete.
    /// Returns the result of the download operation.
    pub async fn coordinate_download<F, Fut>(
        &self,
        repo: &str,
        filename: &str,
        download_fn: F,
    ) -> Result<PathBuf, ModelError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<PathBuf, ModelError>>,
    {
        let lock_path = self.lock_path(repo, filename);
        debug!("Using lock file: {}", lock_path.display());

        // Try to acquire the lock
        match self.try_acquire_lock(&lock_path).await? {
            LockStatus::Acquired(lock_guard) => {
                // We have the lock - do the download
                info!("Acquired download lock for {}/{}", repo, filename);
                let result = download_fn().await;

                // Release lock and record result
                if let Ok(ref path) = result {
                    lock_guard.mark_complete(path)?;
                }
                drop(lock_guard);

                result
            }
            LockStatus::WaitForOther { completed_path } => {
                // Another process completed the download
                info!(
                    "Another process completed download for {}/{}: {}",
                    repo,
                    filename,
                    completed_path.display()
                );
                Ok(completed_path)
            }
        }
    }

    /// Try to acquire the download lock
    async fn try_acquire_lock(&self, lock_path: &Path) -> Result<LockStatus, ModelError> {
        let start = Instant::now();

        loop {
            // Check if lock file exists
            if lock_path.exists() {
                // Check for stale lock
                if self.is_stale_lock(lock_path)? {
                    warn!("Removing stale lock file: {}", lock_path.display());
                    let _ = fs::remove_file(lock_path);
                    continue;
                }

                // Check if download already completed
                if let Some(completed_path) = self.read_completed_path(lock_path)? {
                    if completed_path.exists() {
                        return Ok(LockStatus::WaitForOther { completed_path });
                    }
                }

                // Another process is downloading - wait
                if start.elapsed() > MAX_WAIT_DURATION {
                    return Err(ModelError::Cache(format!(
                        "Timeout waiting for another process to complete download (waited {:?})",
                        MAX_WAIT_DURATION
                    )));
                }

                debug!(
                    "Waiting for another process to complete download ({:?} elapsed)",
                    start.elapsed()
                );
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }

            // Try to create the lock file atomically
            match self.create_lock_file(lock_path) {
                Ok(guard) => return Ok(LockStatus::Acquired(guard)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Race condition - another process created it first
                    debug!("Lock file created by another process, will wait");
                    tokio::time::sleep(POLL_INTERVAL).await;
                    continue;
                }
                Err(e) => {
                    return Err(ModelError::Cache(format!(
                        "Failed to create lock file: {}",
                        e
                    )));
                }
            }
        }
    }

    /// Create a lock file atomically
    fn create_lock_file(&self, lock_path: &Path) -> Result<LockGuard, std::io::Error> {
        // Use O_EXCL for atomic creation
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)?;

        // Write our PID and start time
        let content = format!(
            "pid={}\nstarted={}\nstatus=downloading\n",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        file.write_all(content.as_bytes())?;
        file.sync_all()?;

        Ok(LockGuard {
            path: lock_path.to_path_buf(),
        })
    }

    /// Check if a lock file is stale (left behind by a crashed/killed process).
    ///
    /// Resolves the owning process's liveness (a syscall) and the lock's age,
    /// then defers the decision to the pure [`stale_decision`] so the policy is
    /// deterministically unit-testable.
    fn is_stale_lock(&self, lock_path: &Path) -> Result<bool, ModelError> {
        let metadata = fs::metadata(lock_path)
            .map_err(|e| ModelError::Cache(format!("Failed to read lock metadata: {}", e)))?;

        let modified = metadata.modified().map_err(|e| {
            ModelError::Cache(format!("Failed to get lock modification time: {}", e))
        })?;

        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();

        let content = fs::read_to_string(lock_path).unwrap_or_default();
        let owner_alive = parse_lock_pid(&content).map(process_is_alive);

        Ok(stale_decision(&content, age, owner_alive))
    }

    /// Read the completed file path from a lock file
    fn read_completed_path(&self, lock_path: &Path) -> Result<Option<PathBuf>, ModelError> {
        let content = fs::read_to_string(lock_path)
            .map_err(|e| ModelError::Cache(format!("Failed to read lock file: {}", e)))?;

        if !content.contains("status=completed") {
            return Ok(None);
        }

        for line in content.lines() {
            if let Some(path_str) = line.strip_prefix("path=") {
                return Ok(Some(PathBuf::from(path_str)));
            }
        }

        Ok(None)
    }
}

impl Default for DownloadCoordinator {
    fn default() -> Self {
        Self::new().expect("Failed to create download coordinator")
    }
}

/// Parse the owning `pid=` from a lock file's contents, if present and valid.
fn parse_lock_pid(content: &str) -> Option<i32> {
    content
        .lines()
        .find_map(|l| l.strip_prefix("pid=")?.trim().parse::<i32>().ok())
}

/// Whether `pid` refers to a currently-live process.
///
/// On unix this is `kill(pid, 0)`: `0` means alive; `EPERM` means the process
/// exists but we lack permission to signal it (still alive); `ESRCH` means no
/// such process (dead). On non-unix targets there is no portable, cheap probe,
/// so we report "alive" and let the age-based fallback handle recovery.
#[cfg(unix)]
fn process_is_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if ret == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_is_alive(_pid: i32) -> bool {
    true
}

/// Pure staleness policy for a lock, given its file `content`, its `age`, and
/// whether its owning process is alive (`None` when no parseable PID).
///
/// - A `completed` lock is never stale — it records a finished download.
/// - If the owner is **alive**, the lock is never stale: that process is
///   actively downloading (possibly a large file on a slow link). The overall
///   wait is still bounded by `MAX_WAIT_DURATION` in the acquire loop, so even a
///   genuinely stuck owner cannot wedge a waiter forever.
/// - If the owner is **dead**, the lock is stale once past a short grace
///   (`STALE_CHECK_MIN_AGE_SECS`) — reclaimed in seconds, not minutes.
/// - If the owner is **unknown** (missing/garbled PID), fall back to the hard
///   age threshold.
fn stale_decision(content: &str, age: Duration, owner_alive: Option<bool>) -> bool {
    if content.contains("status=completed") {
        return false;
    }
    match owner_alive {
        Some(true) => false,
        Some(false) => age > Duration::from_secs(STALE_CHECK_MIN_AGE_SECS),
        None => age > STALE_LOCK_THRESHOLD,
    }
}

/// Result of trying to acquire a lock
enum LockStatus {
    /// We acquired the lock and should do the download
    Acquired(LockGuard),
    /// Another process completed the download
    WaitForOther { completed_path: PathBuf },
}

/// RAII guard for the lock file
struct LockGuard {
    path: PathBuf,
}

impl LockGuard {
    /// Mark the download as complete with the final path
    fn mark_complete(&self, downloaded_path: &Path) -> Result<(), ModelError> {
        let content = format!(
            "pid={}\nstarted={}\nstatus=completed\npath={}\n",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            downloaded_path.display()
        );

        fs::write(&self.path, content)
            .map_err(|e| ModelError::Cache(format!("Failed to update lock file: {}", e)))?;

        Ok(())
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Don't remove the lock file - it contains the completion status
        // Other processes need to read it to know the download is done
        // The lock file will be cleaned up on next successful download or
        // will be considered stale after STALE_LOCK_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_coordinator(temp_dir: &TempDir) -> DownloadCoordinator {
        let lock_dir = temp_dir.path().join("locks");
        fs::create_dir_all(&lock_dir).unwrap();
        DownloadCoordinator { lock_dir }
    }

    const DOWNLOADING: &str = "pid=12345\nstarted=100\nstatus=downloading\n";
    const COMPLETED: &str = "pid=12345\nstarted=100\nstatus=completed\npath=/tmp/m.gguf\n";

    // --- pure staleness policy ---------------------------------------------

    #[test]
    fn completed_lock_is_never_stale() {
        // Even old, even if the owner is dead — a completed lock records a
        // finished download and must be reused, not reclaimed.
        assert!(!stale_decision(
            COMPLETED,
            Duration::from_secs(0),
            Some(true)
        ));
        assert!(!stale_decision(
            COMPLETED,
            STALE_LOCK_THRESHOLD * 2,
            Some(false)
        ));
        assert!(!stale_decision(COMPLETED, STALE_LOCK_THRESHOLD * 2, None));
    }

    #[test]
    fn live_owner_is_never_stolen_regardless_of_age() {
        // A live owner may be downloading a huge file on a slow link; never
        // steal its lock. (The acquire loop's MAX_WAIT_DURATION is the backstop.)
        assert!(!stale_decision(
            DOWNLOADING,
            Duration::from_secs(0),
            Some(true)
        ));
        assert!(!stale_decision(
            DOWNLOADING,
            STALE_LOCK_THRESHOLD * 100,
            Some(true)
        ));
    }

    #[test]
    fn dead_owner_is_stale_after_short_grace() {
        // Just-created lock from a dead owner: within grace, not yet stale.
        assert!(!stale_decision(
            DOWNLOADING,
            Duration::from_secs(1),
            Some(false)
        ));
        // Past the short grace: stale — reclaimed in ~10s, not 5 minutes.
        assert!(stale_decision(
            DOWNLOADING,
            Duration::from_secs(STALE_CHECK_MIN_AGE_SECS + 1),
            Some(false)
        ));
    }

    #[test]
    fn unknown_owner_falls_back_to_age_threshold() {
        assert!(!stale_decision(
            DOWNLOADING,
            STALE_LOCK_THRESHOLD - Duration::from_secs(1),
            None
        ));
        assert!(stale_decision(
            DOWNLOADING,
            STALE_LOCK_THRESHOLD + Duration::from_secs(1),
            None
        ));
    }

    #[test]
    fn parse_lock_pid_reads_the_pid() {
        assert_eq!(parse_lock_pid(DOWNLOADING), Some(12345));
        assert_eq!(parse_lock_pid("started=1\nstatus=downloading\n"), None);
        assert_eq!(parse_lock_pid("pid=notanumber\n"), None);
    }

    // --- process liveness primitive ----------------------------------------

    #[test]
    fn process_is_alive_for_self() {
        assert!(process_is_alive(std::process::id() as i32));
    }

    #[test]
    fn process_is_alive_false_for_nonpositive_pid() {
        assert!(!process_is_alive(0));
        assert!(!process_is_alive(-1));
    }

    #[cfg(unix)]
    #[test]
    fn process_is_alive_false_for_reaped_child() {
        // Spawn and reap a child; its PID is then dead.
        let child = std::process::Command::new("true")
            .spawn()
            .expect("spawn `true`");
        let pid = child.id() as i32;
        let mut child = child;
        let _ = child.wait().expect("wait child");
        assert!(
            !process_is_alive(pid),
            "a reaped child PID must read as dead"
        );
    }

    // --- end-to-end is_stale_lock (reads real mtime) -----------------------

    /// Backdate a file's mtime by `secs` so age-based logic can be exercised
    /// without sleeping.
    #[cfg(unix)]
    fn backdate(path: &Path, secs: u64) {
        let when = SystemTime::now() - Duration::from_secs(secs);
        let epoch = when
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as libc::time_t;
        let tv = libc::timeval {
            tv_sec: epoch,
            tv_usec: 0,
        };
        let times = [tv, tv];
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let rc = unsafe { libc::utimes(c.as_ptr(), times.as_ptr()) };
        assert_eq!(rc, 0, "utimes must succeed");
    }

    #[cfg(unix)]
    #[test]
    fn is_stale_lock_reclaims_dead_owner_quickly() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock = coordinator.lock_dir.join("dead.lock");

        // A `downloading` lock owned by a PID that is not alive. Use a reaped
        // child's PID so liveness is deterministically dead.
        let c = std::process::Command::new("true").spawn().unwrap();
        let dead_pid = c.id();
        let mut c = c;
        c.wait().unwrap();
        fs::write(
            &lock,
            format!("pid={dead_pid}\nstarted=1\nstatus=downloading\n"),
        )
        .unwrap();
        // Older than the short grace but FAR younger than the 5-min hard
        // threshold — pre-fix this would have waited ~5 minutes.
        backdate(&lock, STALE_CHECK_MIN_AGE_SECS + 5);

        assert!(
            coordinator.is_stale_lock(&lock).unwrap(),
            "a dead-owner downloading lock past the grace must be stale"
        );
    }

    #[cfg(unix)]
    #[test]
    fn is_stale_lock_keeps_live_owner_even_when_old() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock = coordinator.lock_dir.join("live.lock");

        // Owned by THIS process (alive), and deliberately older than the hard
        // age threshold: must NOT be treated as stale (it's actively working).
        fs::write(
            &lock,
            format!(
                "pid={}\nstarted=1\nstatus=downloading\n",
                std::process::id()
            ),
        )
        .unwrap();
        backdate(&lock, STALE_LOCK_THRESHOLD_SECS + 60);

        assert!(
            !coordinator.is_stale_lock(&lock).unwrap(),
            "a live-owner lock must never be reclaimed, even when old"
        );
    }

    #[tokio::test]
    async fn test_single_download() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let download_path = temp_dir.path().join("model.gguf");

        // Create a fake downloaded file
        fs::write(&download_path, "fake model data").unwrap();

        let result = coordinator
            .coordinate_download("test/repo", "model.gguf", || async {
                Ok(download_path.clone())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), download_path);
    }

    #[tokio::test]
    async fn test_lock_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);

        let path1 = coordinator.lock_path("owner/repo", "model.gguf");
        let path2 = coordinator.lock_path("owner/repo", "other.gguf");

        assert_ne!(path1, path2);
        assert!(path1.to_string_lossy().contains("owner--repo"));
    }

    #[test]
    fn test_create_lock_file() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");

        let guard = coordinator.create_lock_file(&lock_path).unwrap();
        assert!(lock_path.exists());

        let content = fs::read_to_string(&lock_path).unwrap();
        assert!(content.contains("status=downloading"));
        assert!(content.contains(&format!("pid={}", std::process::id())));

        drop(guard);
    }

    #[test]
    fn test_mark_complete() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");
        let model_path = PathBuf::from("/path/to/model.gguf");

        let guard = coordinator.create_lock_file(&lock_path).unwrap();
        guard.mark_complete(&model_path).unwrap();

        let content = fs::read_to_string(&lock_path).unwrap();
        assert!(content.contains("status=completed"));
        assert!(content.contains(&model_path.display().to_string()));
    }

    #[test]
    fn test_read_completed_path() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");
        let model_path = PathBuf::from("/path/to/model.gguf");

        // Write a completed lock file
        fs::write(
            &lock_path,
            format!("status=completed\npath={}\n", model_path.display()),
        )
        .unwrap();

        let result = coordinator.read_completed_path(&lock_path).unwrap();
        assert_eq!(result, Some(model_path));
    }

    #[test]
    fn test_read_incomplete_path() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");

        // Write an incomplete lock file
        fs::write(&lock_path, "status=downloading\n").unwrap();

        let result = coordinator.read_completed_path(&lock_path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_completed_path_without_path_line() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");

        // Completed status but no path= line
        fs::write(&lock_path, "status=completed\n").unwrap();

        let result = coordinator.read_completed_path(&lock_path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_is_stale_lock_fresh_downloading() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");

        // Fresh lock file with downloading status
        fs::write(&lock_path, "pid=12345\nstatus=downloading\n").unwrap();

        let is_stale = coordinator.is_stale_lock(&lock_path).unwrap();
        assert!(!is_stale, "Fresh lock should not be stale");
    }

    #[test]
    fn test_is_stale_lock_completed_never_stale() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("test.lock");

        // Completed lock file — should never be considered stale
        fs::write(
            &lock_path,
            "pid=12345\nstatus=completed\npath=/some/model.gguf\n",
        )
        .unwrap();

        let is_stale = coordinator.is_stale_lock(&lock_path).unwrap();
        assert!(!is_stale, "Completed lock should not be stale");
    }

    #[test]
    fn test_lock_path_safe_characters() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);

        // Repo with special characters gets sanitized
        let path = coordinator.lock_path("org/repo-name", "model-v2.gguf");
        let filename = path.file_name().unwrap().to_string_lossy();
        assert!(
            filename.ends_with(".lock"),
            "Lock path should end with .lock"
        );
        // Should not contain forward slashes
        assert!(
            !filename.contains('/'),
            "Lock filename should not contain slashes"
        );
    }

    #[test]
    fn test_lock_path_deterministic() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);

        let path1 = coordinator.lock_path("org/repo", "model.gguf");
        let path2 = coordinator.lock_path("org/repo", "model.gguf");
        assert_eq!(path1, path2, "Same input should yield same lock path");
    }

    #[test]
    fn test_create_lock_file_atomic_exclusion() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let lock_path = coordinator.lock_dir.join("exclusive.lock");

        // First creation succeeds
        let _guard = coordinator.create_lock_file(&lock_path).unwrap();

        // Second creation should fail (AlreadyExists)
        let result = coordinator.create_lock_file(&lock_path);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    }

    #[tokio::test]
    async fn test_coordinate_download_error_propagation() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);

        let result = coordinator
            .coordinate_download("test/repo", "model.gguf", || async {
                Err(ModelError::Network("download failed".to_string()))
            })
            .await;

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("download failed"));
    }

    #[tokio::test]
    async fn test_coordinate_download_completed_reuse() {
        let temp_dir = TempDir::new().unwrap();
        let coordinator = test_coordinator(&temp_dir);
        let download_path = temp_dir.path().join("model.gguf");
        fs::write(&download_path, "fake model").unwrap();

        // First download succeeds and marks complete
        let result = coordinator
            .coordinate_download("test/repo", "model.gguf", || async {
                Ok(download_path.clone())
            })
            .await;
        assert!(result.is_ok());

        // Second coordinate should find the completed lock and return the path
        let result2 = coordinator
            .coordinate_download("test/repo", "model.gguf", || async {
                panic!("Should not be called — previous download completed");
            })
            .await;
        assert!(result2.is_ok());
        assert_eq!(result2.unwrap(), download_path);
    }

    #[test]
    fn test_default_coordinator() {
        // DownloadCoordinator::default() should not panic
        let coord = DownloadCoordinator::default();
        // lock_dir may or may not exist yet — just verify default() doesn't panic
        let _ = coord.lock_dir.exists();
    }

    #[test]
    fn test_default_lock_dir_returns_path() {
        let result = DownloadCoordinator::default_lock_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("download_locks"));
    }
}
