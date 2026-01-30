//! File-lock based leader election
//!
//! Uses file locking to ensure only one process becomes the leader for a given workspace.
//! The first process to acquire the lock becomes the leader and holds the index in memory.
//! Other processes become clients and connect to the leader via Unix socket.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;

/// Leader election coordinator
///
/// Provides file-lock based leader election for a workspace. The lock file and socket
/// paths are derived from a hash of the workspace root path.
pub struct LeaderElection {
    /// Path to the lock file
    lock_path: PathBuf,
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Original workspace root
    workspace_root: PathBuf,
}

/// Guard that holds the leader lock
///
/// When dropped, releases the lock and cleans up the socket file.
pub struct LeaderGuard {
    /// The lock file handle (lock released on drop)
    _lock_file: File,
    /// Path to socket file (cleaned up on drop)
    socket_path: PathBuf,
}

/// Errors that can occur during election
#[derive(Debug)]
pub enum ElectionError {
    /// Failed to create lock file
    LockFileCreation(io::Error),
    /// Lock is held by another process
    LockHeld,
    /// Failed to acquire lock
    LockAcquisition(io::Error),
    /// Socket path error
    SocketError(io::Error),
}

impl std::fmt::Display for ElectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LockFileCreation(e) => write!(f, "Failed to create lock file: {}", e),
            Self::LockHeld => write!(f, "Lock is held by another process"),
            Self::LockAcquisition(e) => write!(f, "Failed to acquire lock: {}", e),
            Self::SocketError(e) => write!(f, "Socket error: {}", e),
        }
    }
}

impl std::error::Error for ElectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LockFileCreation(e) => Some(e),
            Self::LockHeld => None,
            Self::LockAcquisition(e) => Some(e),
            Self::SocketError(e) => Some(e),
        }
    }
}

impl LeaderElection {
    /// Create a new election coordinator for a workspace
    ///
    /// The lock and socket paths are derived from a hash of the workspace root,
    /// stored in the system temp directory.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let hash = Self::hash_path(&workspace_root);

        // Use /tmp on Unix for lock and socket files
        let base = std::env::temp_dir();

        Self {
            lock_path: base.join(format!("sah-ts-{}.lock", hash)),
            socket_path: base.join(format!("sah-ts-{}.sock", hash)),
            workspace_root,
        }
    }

    /// Compute a short hash of a path for unique identification
    fn hash_path(path: &Path) -> String {
        let path_str = path.to_string_lossy();
        let digest = md5::compute(path_str.as_bytes());
        format!("{:x}", digest)[..12].to_string()
    }

    /// Try to become the leader
    ///
    /// Attempts to acquire an exclusive lock on the lock file.
    /// Returns a `LeaderGuard` if successful, or `ElectionError::LockHeld` if another
    /// process already holds the lock.
    pub fn try_become_leader(&self) -> Result<LeaderGuard, ElectionError> {
        // Ensure parent directory exists
        if let Some(parent) = self.lock_path.parent() {
            fs::create_dir_all(parent).map_err(ElectionError::LockFileCreation)?;
        }

        // Create or open lock file
        let lock_file = File::create(&self.lock_path).map_err(ElectionError::LockFileCreation)?;

        // Try to acquire exclusive lock (non-blocking)
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // Clean up any stale socket file from previous run
                let _ = fs::remove_file(&self.socket_path);

                Ok(LeaderGuard {
                    _lock_file: lock_file,
                    socket_path: self.socket_path.clone(),
                })
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(ElectionError::LockHeld),
            Err(e) => Err(ElectionError::LockAcquisition(e)),
        }
    }

    /// Check if a leader is currently active
    ///
    /// Returns true if the socket file exists (indicating a leader is running).
    pub fn leader_exists(&self) -> bool {
        self.socket_path.exists()
    }

    /// Get the path to the Unix socket
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get the path to the lock file
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Get the workspace root
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl Drop for LeaderGuard {
    fn drop(&mut self) {
        // Clean up socket file when leader exits
        let _ = fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use tempfile::TempDir;

    #[test]
    fn test_election_new() {
        let election = LeaderElection::new("/some/workspace");

        assert!(election.lock_path().to_string_lossy().contains("sah-ts-"));
        assert!(election.socket_path().to_string_lossy().contains("sah-ts-"));
        assert_eq!(election.workspace_root(), Path::new("/some/workspace"));
    }

    #[test]
    fn test_hash_path_deterministic() {
        let hash1 = LeaderElection::hash_path(Path::new("/workspace/project"));
        let hash2 = LeaderElection::hash_path(Path::new("/workspace/project"));
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_path_different_for_different_paths() {
        let hash1 = LeaderElection::hash_path(Path::new("/workspace/a"));
        let hash2 = LeaderElection::hash_path(Path::new("/workspace/b"));
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_try_become_leader_success() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        let guard = election.try_become_leader();
        assert!(guard.is_ok());
    }

    #[test]
    fn test_try_become_leader_lock_held() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        // First process acquires lock
        let _guard1 = election.try_become_leader().unwrap();

        // Second attempt should fail
        let result = election.try_become_leader();
        assert!(matches!(result, Err(ElectionError::LockHeld)));
    }

    #[test]
    fn test_leader_guard_cleanup() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        // Create a fake socket file to verify cleanup
        fs::write(election.socket_path(), "test").unwrap();
        assert!(election.socket_path().exists());

        {
            // Acquire and hold lock
            let _guard = election.try_become_leader().unwrap();
            // Socket should be cleaned up on acquiring lock
            assert!(!election.socket_path().exists());
        }

        // After guard is dropped, socket should still be cleaned up
        assert!(!election.socket_path().exists());
    }

    #[test]
    fn test_leader_exists() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        assert!(!election.leader_exists());

        // Create socket file to simulate active leader
        fs::write(election.socket_path(), "").unwrap();
        assert!(election.leader_exists());
    }

    #[test]
    fn test_election_error_display() {
        let err = ElectionError::LockHeld;
        assert_eq!(format!("{}", err), "Lock is held by another process");

        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = ElectionError::LockFileCreation(io_err);
        assert!(format!("{}", err).contains("Failed to create lock file"));
    }

    #[test]
    fn test_election_error_source() {
        let err = ElectionError::LockHeld;
        assert!(err.source().is_none());

        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = ElectionError::LockFileCreation(io_err);
        assert!(err.source().is_some());
    }
}
