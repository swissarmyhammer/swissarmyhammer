//! File-lock based leader election
//!
//! Uses file locking to ensure only one process becomes the leader for a given workspace.
//! The first process to acquire the lock becomes the leader.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::error::{ElectionError, Result};

/// Default prefix for lock and socket files
const DEFAULT_PREFIX: &str = "sah";

/// Configuration for leader election
#[derive(Debug, Clone)]
pub struct ElectionConfig {
    /// Prefix for lock/socket file names (default: "sah")
    pub prefix: String,
    /// Base directory for lock/socket files (default: system temp dir)
    pub base_dir: Option<PathBuf>,
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PREFIX.to_string(),
            base_dir: None,
        }
    }
}

impl ElectionConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the prefix for lock/socket files
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set the base directory for lock/socket files
    pub fn with_base_dir(mut self, base_dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(base_dir.into());
        self
    }

    /// Get the base directory (uses system temp dir if not set)
    fn base_dir(&self) -> PathBuf {
        self.base_dir
            .clone()
            .unwrap_or_else(std::env::temp_dir)
    }
}

/// Leader election coordinator
///
/// Provides file-lock based leader election for a workspace. The lock file and socket
/// paths are derived from a hash of the workspace root path.
///
/// # Example
///
/// ```ignore
/// use swissarmyhammer_leader_election::{LeaderElection, ElectionConfig};
///
/// let election = LeaderElection::new("/workspace/path");
///
/// match election.try_become_leader() {
///     Ok(guard) => {
///         // We are the leader - guard holds the lock
///         println!("Became leader, socket at: {:?}", election.socket_path());
///     }
///     Err(ElectionError::LockHeld) => {
///         // Another process is the leader
///         println!("Another process is the leader");
///     }
/// }
/// ```
pub struct LeaderElection {
    /// Path to the lock file
    lock_path: PathBuf,
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Original workspace root
    workspace_root: PathBuf,
}

impl LeaderElection {
    /// Create a new election coordinator for a workspace with default config
    ///
    /// The lock and socket paths are derived from a hash of the workspace root,
    /// stored in the system temp directory.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self::with_config(workspace_root, ElectionConfig::default())
    }

    /// Create a new election coordinator with custom configuration
    pub fn with_config(workspace_root: impl AsRef<Path>, config: ElectionConfig) -> Self {
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let hash = Self::hash_path(&workspace_root);
        let base = config.base_dir();

        Self {
            lock_path: base.join(format!("{}-ts-{}.lock", config.prefix, hash)),
            socket_path: base.join(format!("{}-ts-{}.sock", config.prefix, hash)),
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
    pub fn try_become_leader(&self) -> Result<LeaderGuard> {
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

/// Guard that holds the leader lock
///
/// When dropped, releases the lock and cleans up the socket file.
/// The lock is held as long as this guard exists.
pub struct LeaderGuard {
    /// The lock file handle (lock released on drop)
    _lock_file: File,
    /// Path to socket file (cleaned up on drop)
    socket_path: PathBuf,
}

impl LeaderGuard {
    /// Get the socket path this guard will clean up on drop
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
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
    use tempfile::TempDir;

    #[test]
    fn test_election_config_default() {
        let config = ElectionConfig::default();
        assert_eq!(config.prefix, DEFAULT_PREFIX);
        assert!(config.base_dir.is_none());
    }

    #[test]
    fn test_election_config_builder() {
        let config = ElectionConfig::new()
            .with_prefix("myapp")
            .with_base_dir("/custom/dir");

        assert_eq!(config.prefix, "myapp");
        assert_eq!(config.base_dir, Some(PathBuf::from("/custom/dir")));
    }

    #[test]
    fn test_election_new() {
        let election = LeaderElection::new("/some/workspace");

        assert!(election.lock_path().to_string_lossy().contains("sah-ts-"));
        assert!(election.socket_path().to_string_lossy().contains("sah-ts-"));
        assert_eq!(election.workspace_root(), Path::new("/some/workspace"));
    }

    #[test]
    fn test_election_with_custom_config() {
        let config = ElectionConfig::new().with_prefix("custom");
        let election = LeaderElection::with_config("/some/workspace", config);

        assert!(election
            .lock_path()
            .to_string_lossy()
            .contains("custom-ts-"));
        assert!(election
            .socket_path()
            .to_string_lossy()
            .contains("custom-ts-"));
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
    fn test_leader_guard_socket_path() {
        let dir = TempDir::new().unwrap();
        let election = LeaderElection::new(dir.path());

        let guard = election.try_become_leader().unwrap();
        assert_eq!(guard.socket_path(), election.socket_path());
    }

    #[test]
    fn test_election_with_custom_base_dir() {
        let dir = TempDir::new().unwrap();
        let config = ElectionConfig::new().with_base_dir(dir.path());
        let election = LeaderElection::with_config("/workspace", config);

        assert!(election.lock_path().starts_with(dir.path()));
        assert!(election.socket_path().starts_with(dir.path()));
    }
}
