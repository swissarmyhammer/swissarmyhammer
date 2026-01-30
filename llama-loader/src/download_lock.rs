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

/// Minimum age in seconds before checking if owning process is dead
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
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
        fs::create_dir_all(&lock_dir).map_err(|e| {
            ModelError::Cache(format!("Failed to create lock directory: {}", e))
        })?;
        Ok(Self { lock_dir })
    }

    /// Get the default lock directory
    fn default_lock_dir() -> Result<PathBuf, ModelError> {
        // Use HuggingFace cache directory structure
        let cache_dir = std::env::var("HF_HOME")
            .or_else(|_| {
                std::env::var("XDG_CACHE_HOME").map(|p| format!("{}/huggingface", p))
            })
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
        let safe_name = format!("{}_{}", repo.replace('/', "--"), filename)
            .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.', "_");
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

    /// Check if a lock file is stale (from a crashed process)
    fn is_stale_lock(&self, lock_path: &Path) -> Result<bool, ModelError> {
        let metadata = fs::metadata(lock_path).map_err(|e| {
            ModelError::Cache(format!("Failed to read lock metadata: {}", e))
        })?;

        let modified = metadata.modified().map_err(|e| {
            ModelError::Cache(format!("Failed to get lock modification time: {}", e))
        })?;

        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();

        // Check the lock file content
        if let Ok(content) = fs::read_to_string(lock_path) {
            // Check if it says "completed" - not stale if completed
            if content.contains("status=completed") {
                return Ok(false);
            }

            // Check if the process is still alive (Linux only - uses /proc)
            #[cfg(target_os = "linux")]
            if let Some(pid_line) = content.lines().find(|l| l.starts_with("pid=")) {
                if let Ok(pid) = pid_line.trim_start_matches("pid=").parse::<i32>() {
                    // Check if process exists by looking at /proc
                    let proc_path = format!("/proc/{}", pid);
                    if !std::path::Path::new(&proc_path).exists()
                        && age > Duration::from_secs(STALE_CHECK_MIN_AGE_SECS)
                    {
                        return Ok(true);
                    }
                }
            }

            // On macOS, we rely primarily on the age-based check
            // since /proc doesn't exist
        }

        Ok(age > STALE_LOCK_THRESHOLD)
    }

    /// Read the completed file path from a lock file
    fn read_completed_path(&self, lock_path: &Path) -> Result<Option<PathBuf>, ModelError> {
        let content = fs::read_to_string(lock_path).map_err(|e| {
            ModelError::Cache(format!("Failed to read lock file: {}", e))
        })?;

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

        fs::write(&self.path, content).map_err(|e| {
            ModelError::Cache(format!("Failed to update lock file: {}", e))
        })?;

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
}
