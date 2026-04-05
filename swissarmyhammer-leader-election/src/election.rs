//! File-lock based leader election
//!
//! Uses file locking (`flock`) to coordinate leader election across multiple processes.
//! The first process to acquire the lock becomes the leader; others become followers.
//! Followers can re-contest the election at any time via `try_promote()`.
//!
//! The OS automatically releases `flock` when a process exits or crashes, so a
//! follower calling `try_promote()` will win the election once the old leader is gone.

use std::fmt;
use std::fs::{self, File};
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::bus::{BusMessage, NullMessage, Publisher, Subscriber};
use crate::discovery::{self, BusAddresses};
use crate::error::{ElectionError, Result};
use crate::proxy::ProxyHandle;

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
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
    }
}

/// The outcome of a leader election attempt.
///
/// Callers match on this to determine whether they won (Leader) or lost
/// (Follower). A `FollowerGuard` can re-contest the election later via
/// `try_promote()`.
///
/// The default type parameter `NullMessage` means existing consumers that don't
/// specify a message type continue to work unchanged.
pub enum ElectionOutcome<M: BusMessage = NullMessage> {
    /// This process won the election and holds the leader lock.
    Leader(LeaderGuard<M>),
    /// Another process holds the lock. The follower can retry later.
    Follower(FollowerGuard<M>),
}

impl<M: BusMessage> ElectionOutcome<M> {
    /// Publish a message to the bus, regardless of leader/follower role.
    pub fn publish(&self, msg: &M) -> Result<()> {
        match self {
            Self::Leader(guard) => guard.publish(msg),
            Self::Follower(guard) => guard.publish(msg),
        }
    }
}

impl<M: BusMessage> fmt::Debug for ElectionOutcome<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leader(_) => f.write_str("ElectionOutcome::Leader"),
            Self::Follower(_) => f.write_str("ElectionOutcome::Follower"),
        }
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
/// use swissarmyhammer_leader_election::LeaderElection;
///
/// let election = LeaderElection::new("/workspace/path");
///
/// match election.elect() {
///     Ok(ElectionOutcome::Leader(guard)) => {
///         // We are the leader — guard holds the lock
///     }
///     Ok(ElectionOutcome::Follower(follower)) => {
///         // Another process is leader — try again later
///         // if let Some(guard) = follower.try_promote()? { ... }
///     }
///     Err(e) => eprintln!("Election failed: {}", e),
/// }
/// ```
pub struct LeaderElection<M: BusMessage = NullMessage> {
    /// Path to the lock file
    lock_path: PathBuf,
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Original workspace root
    workspace_root: PathBuf,
    /// Configuration (stored for bus address computation)
    config: ElectionConfig,
    /// Workspace hash (stored for bus address computation)
    hash: String,
    _phantom: PhantomData<M>,
}

/// Compute a hash of a path for unique identification.
///
/// Uses the full 32-character MD5 hex digest to avoid collision risk
/// when two different workspace paths share a lock file.
fn hash_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    let digest = md5::compute(path_str.as_bytes());
    format!("{:x}", digest)
}

impl<M: BusMessage> LeaderElection<M> {
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
        let hash = hash_path(&workspace_root);
        let base = config.base_dir();

        Self {
            lock_path: base.join(format!("{}-ts-{}.lock", config.prefix, hash)),
            socket_path: base.join(format!("{}-ts-{}.sock", config.prefix, hash)),
            workspace_root,
            config: config.clone(),
            hash,
            _phantom: PhantomData,
        }
    }

    /// Get the bus addresses for this election.
    fn bus_addresses(&self) -> BusAddresses {
        discovery::ipc_addresses(&self.config.base_dir(), &self.config.prefix, &self.hash)
    }

    /// Get the discovery file path for this election.
    fn discovery_path(&self) -> PathBuf {
        discovery::discovery_path(&self.config.base_dir(), &self.config.prefix, &self.hash)
    }

    /// Run the election. Returns `Leader` or `Follower` outcome.
    ///
    /// This is the primary entry point. Use this instead of `try_become_leader()`
    /// for new code — it returns a typed outcome so followers can re-contest later.
    pub fn elect(&self) -> Result<ElectionOutcome<M>> {
        match self.try_acquire_lock() {
            Ok(guard) => Ok(ElectionOutcome::Leader(guard)),
            Err(ElectionError::LockHeld) => {
                // Read discovery file to find proxy addresses
                let disc_path = self.discovery_path();
                // Create context once; reused for both the publisher and all future subscribe() calls
                let ctx = zmq::Context::new();
                let publisher = match discovery::read_discovery(&disc_path)? {
                    Some(addrs) => Publisher::connected(&ctx, &addrs.frontend)?,
                    None => Publisher::noop(),
                };
                Ok(ElectionOutcome::Follower(FollowerGuard {
                    lock_path: self.lock_path.clone(),
                    socket_path: self.socket_path.clone(),
                    discovery_path: disc_path,
                    bus_addresses: self.bus_addresses(),
                    publisher,
                    zmq_ctx: ctx,
                }))
            }
            Err(e) => Err(e),
        }
    }

    /// Try to become the leader (legacy API, prefer `elect()`).
    ///
    /// Attempts to acquire an exclusive lock on the lock file.
    /// Returns a `LeaderGuard` if successful, or `ElectionError::LockHeld` if another
    /// process already holds the lock.
    pub fn try_become_leader(&self) -> Result<LeaderGuard<M>> {
        self.try_acquire_lock()
    }

    /// Internal: attempt to acquire the flock.
    fn try_acquire_lock(&self) -> Result<LeaderGuard<M>> {
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

                // Start the bus proxy
                let addrs = self.bus_addresses();
                let disc_path = self.discovery_path();

                // Clean up stale IPC sockets before binding
                discovery::cleanup_discovery(&disc_path, &addrs);

                let proxy = ProxyHandle::start(&addrs)?;
                discovery::write_discovery(&disc_path, &addrs)?;

                // Create a publisher connected to our own proxy
                let publisher = Publisher::connected(proxy.zmq_context(), &addrs.frontend)?;

                Ok(LeaderGuard {
                    _lock_file: lock_file,
                    lock_path: self.lock_path.clone(),
                    socket_path: self.socket_path.clone(),
                    discovery_path: disc_path,
                    bus_addresses: addrs,
                    _proxy: proxy,
                    publisher,
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

    /// Check if the leader lock is currently held by another process.
    ///
    /// Opens the lock file read-only and attempts a non-blocking exclusive lock.
    /// If the lock cannot be acquired, another process holds it (i.e. indexing
    /// is in progress). Returns false if the file doesn't exist or the lock is
    /// free.
    pub fn is_locked(&self) -> bool {
        let Ok(file) = File::open(&self.lock_path) else {
            return false;
        };
        match file.try_lock_exclusive() {
            Ok(()) => {
                // Lock was free — unlock immediately
                let _ = file.unlock();
                false
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        }
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

/// Guard that holds the leader lock.
///
/// When dropped, releases the flock, cleans up the socket file, and removes
/// the lock file from disk.
pub struct LeaderGuard<M: BusMessage = NullMessage> {
    /// The lock file handle (flock released on drop via fs2/OS)
    _lock_file: File,
    /// Path to lock file (cleaned up on drop)
    lock_path: PathBuf,
    /// Path to socket file (cleaned up on drop)
    socket_path: PathBuf,
    /// Path to discovery file (cleaned up on drop)
    discovery_path: PathBuf,
    /// Bus addresses (for cleanup)
    bus_addresses: BusAddresses,
    /// Proxy handle (stopped on drop)
    _proxy: ProxyHandle,
    /// Publisher for sending messages to the bus
    publisher: Publisher<M>,
}

impl<M: BusMessage> fmt::Debug for LeaderGuard<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeaderGuard")
            .field("lock_path", &self.lock_path)
            .finish()
    }
}

impl<M: BusMessage> LeaderGuard<M> {
    /// Get the socket path this guard will clean up on drop
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Publish a message to the bus.
    pub fn publish(&self, msg: &M) -> Result<()> {
        self.publisher.send(msg)
    }

    /// Subscribe to messages on the bus, optionally filtered by topics.
    ///
    /// Pass empty slice to subscribe to all messages. Reuses the ZMQ context
    /// from the proxy handle rather than allocating a new heavyweight context.
    pub fn subscribe(&self, topics: &[&[u8]]) -> Result<Subscriber<M>> {
        Subscriber::connected(
            self._proxy.zmq_context(),
            &self.bus_addresses.backend,
            topics,
        )
    }

    /// Get the bus addresses (for external subscribers).
    pub fn bus_addresses(&self) -> &BusAddresses {
        &self.bus_addresses
    }
}

impl<M: BusMessage> Drop for LeaderGuard<M> {
    fn drop(&mut self) {
        // Clean up discovery file and IPC sockets
        discovery::cleanup_discovery(&self.discovery_path, &self.bus_addresses);
        // Clean up socket and lock files when leader exits.
        // The flock is released automatically when _lock_file is dropped,
        // but the 0-byte files linger on disk unless we remove them.
        let _ = fs::remove_file(&self.socket_path);
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Guard held by a process that lost the election.
///
/// Call `try_promote()` to re-contest the election. If the previous leader
/// has exited (releasing its flock), this process wins and gets a `LeaderGuard`.
pub struct FollowerGuard<M: BusMessage = NullMessage> {
    /// Path to the lock file (same as the leader's)
    lock_path: PathBuf,
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Path to the discovery file
    discovery_path: PathBuf,
    /// Bus addresses (for creating proxy on promotion)
    bus_addresses: BusAddresses,
    /// Publisher for sending messages to the bus
    publisher: Publisher<M>,
    /// ZMQ context reused for subscribe() calls (created once at election time)
    zmq_ctx: zmq::Context,
}

impl<M: BusMessage> fmt::Debug for FollowerGuard<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FollowerGuard")
            .field("lock_path", &self.lock_path)
            .finish()
    }
}

impl<M: BusMessage> FollowerGuard<M> {
    /// Re-contest the election.
    ///
    /// If the leader's flock has been released (process exited or crashed),
    /// acquires the lock and returns `Ok(Some(LeaderGuard))`.
    /// If another process still holds the lock, returns `Ok(None)`.
    pub fn try_promote(&self) -> Result<Option<LeaderGuard<M>>> {
        // Create-or-open the lock file. The previous leader's drop may have
        // deleted it, so we must use create() to ensure it exists for flock.
        let lock_file = File::create(&self.lock_path).map_err(ElectionError::LockFileCreation)?;

        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // Won the re-election — clean up stale socket from dead leader
                let _ = fs::remove_file(&self.socket_path);

                // Start a new proxy
                let addrs = &self.bus_addresses;
                discovery::cleanup_discovery(&self.discovery_path, addrs);

                let proxy = ProxyHandle::start(addrs)?;
                discovery::write_discovery(&self.discovery_path, addrs)?;

                let publisher = Publisher::connected(proxy.zmq_context(), &addrs.frontend)?;

                Ok(Some(LeaderGuard {
                    _lock_file: lock_file,
                    lock_path: self.lock_path.clone(),
                    socket_path: self.socket_path.clone(),
                    discovery_path: self.discovery_path.clone(),
                    bus_addresses: addrs.clone(),
                    _proxy: proxy,
                    publisher,
                }))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(ElectionError::LockAcquisition(e)),
        }
    }

    /// Publish a message to the bus.
    pub fn publish(&self, msg: &M) -> Result<()> {
        self.publisher.send(msg)
    }

    /// Subscribe to messages on the bus, optionally filtered by topics.
    ///
    /// Pass empty slice to subscribe to all messages. Reuses the ZMQ context
    /// stored on the guard (created once at election time) rather than
    /// allocating a new heavyweight context on every call.
    pub fn subscribe(&self, topics: &[&[u8]]) -> Result<Subscriber<M>> {
        Subscriber::connected(&self.zmq_ctx, &self.bus_addresses.backend, topics)
    }

    /// Get the path to the lock file
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }

    /// Get the path to the socket file
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

// Compile-time assertions: these types are held in Arc<Mutex<>> and sent
// across tokio::spawn boundaries. Catch regressions if a non-Send field
// is ever added.
const _: () = {
    fn _assert_send<T: Send>() {}
    fn _checks() {
        _assert_send::<LeaderElection>();
        _assert_send::<LeaderGuard>();
        _assert_send::<FollowerGuard>();
    }
};

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
        let election: LeaderElection = LeaderElection::new("/some/workspace");

        assert!(election.lock_path().to_string_lossy().contains("sah-ts-"));
        assert!(election.socket_path().to_string_lossy().contains("sah-ts-"));
        assert_eq!(election.workspace_root(), Path::new("/some/workspace"));
    }

    #[test]
    fn test_election_with_custom_config() {
        let config = ElectionConfig::new().with_prefix("custom");
        let election: LeaderElection = LeaderElection::with_config("/some/workspace", config);

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
        let hash1 = hash_path(Path::new("/workspace/project"));
        let hash2 = hash_path(Path::new("/workspace/project"));
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_path_different_for_different_paths() {
        let hash1 = hash_path(Path::new("/workspace/a"));
        let hash2 = hash_path(Path::new("/workspace/b"));
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_try_become_leader_success() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        let guard = election.try_become_leader();
        assert!(guard.is_ok());
    }

    #[test]
    fn test_try_become_leader_lock_held() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // First process acquires lock
        let _guard1 = election.try_become_leader().unwrap();

        // Second attempt should fail
        let result = election.try_become_leader();
        assert!(matches!(result, Err(ElectionError::LockHeld)));
    }

    #[test]
    fn test_leader_guard_cleanup() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

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
        // Lock file should also be cleaned up
        assert!(!election.lock_path().exists());
    }

    #[test]
    fn test_leader_exists() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        assert!(!election.leader_exists());

        // Create socket file to simulate active leader
        fs::write(election.socket_path(), "").unwrap();
        assert!(election.leader_exists());
    }

    #[test]
    fn test_leader_guard_socket_path() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        let guard = election.try_become_leader().unwrap();
        assert_eq!(guard.socket_path(), election.socket_path());
    }

    #[test]
    fn test_election_with_custom_base_dir() {
        let dir = TempDir::new().unwrap();
        let config = ElectionConfig::new().with_base_dir(dir.path());
        let election: LeaderElection = LeaderElection::with_config("/workspace", config);

        assert!(election.lock_path().starts_with(dir.path()));
        assert!(election.socket_path().starts_with(dir.path()));
    }

    // --- New tests for elect() and FollowerGuard ---

    #[test]
    fn test_elect_returns_leader() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        let outcome = election.elect().unwrap();
        assert!(matches!(outcome, ElectionOutcome::Leader(_)));
    }

    #[test]
    fn test_elect_returns_follower_when_lock_held() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // First caller wins
        let _leader = election.try_become_leader().unwrap();

        // Second caller gets follower
        let outcome = election.elect().unwrap();
        assert!(matches!(outcome, ElectionOutcome::Follower(_)));
    }

    #[test]
    fn test_follower_try_promote_fails_while_leader_alive() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        let _leader = election.try_become_leader().unwrap();

        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };

        // Leader still alive — promotion should fail
        let promoted = follower.try_promote().unwrap();
        assert!(promoted.is_none());
    }

    #[test]
    fn test_follower_try_promote_succeeds_after_leader_drops() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // Leader wins, then dies
        let leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };

        // Leader exits — flock released
        drop(leader);

        // Follower promotes
        let promoted = follower.try_promote().unwrap();
        assert!(promoted.is_some());
    }

    #[test]
    fn test_leader_drop_cleans_up_lock_file() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        let guard = election.try_become_leader().unwrap();
        assert!(election.lock_path().exists());

        drop(guard);
        assert!(!election.lock_path().exists());
    }

    #[test]
    fn test_promoted_leader_behaves_like_original() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // First leader wins and dies
        let leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        drop(leader);

        // Follower promotes to leader
        let new_leader = follower.try_promote().unwrap().unwrap();

        // A third process should see the lock as held
        let outcome2 = election.elect().unwrap();
        assert!(matches!(outcome2, ElectionOutcome::Follower(_)));

        // New leader drops — lock file cleaned up
        drop(new_leader);
        assert!(!election.lock_path().exists());
    }

    #[test]
    fn test_election_outcome_debug_leader() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let outcome = election.elect().unwrap();
        let debug = format!("{:?}", outcome);
        assert_eq!(debug, "ElectionOutcome::Leader");
    }

    #[test]
    fn test_election_outcome_debug_follower() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let debug = format!("{:?}", outcome);
        assert_eq!(debug, "ElectionOutcome::Follower");
    }

    #[test]
    fn test_leader_guard_debug() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let debug = format!("{:?}", guard);
        assert!(debug.contains("LeaderGuard"));
        assert!(debug.contains("lock_path"));
    }

    #[test]
    fn test_follower_guard_debug() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        let debug = format!("{:?}", follower);
        assert!(debug.contains("FollowerGuard"));
        assert!(debug.contains("lock_path"));
    }

    #[test]
    fn test_is_locked_returns_true_when_held() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _guard = election.try_become_leader().unwrap();
        assert!(election.is_locked());
    }

    #[test]
    fn test_is_locked_returns_false_when_free() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // Create the lock file without holding the flock
        fs::create_dir_all(election.lock_path().parent().unwrap()).unwrap();
        File::create(election.lock_path()).unwrap();

        assert!(!election.is_locked());
    }

    #[test]
    fn test_is_locked_returns_false_when_no_file() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        // No lock file exists yet
        assert!(!election.is_locked());
    }

    #[test]
    fn test_is_locked_returns_false_after_leader_drops() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        assert!(election.is_locked());
        drop(guard);
        // Lock file is removed on drop, so is_locked returns false (no file)
        assert!(!election.is_locked());
    }

    #[test]
    fn test_leader_guard_publish_noop() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        // NullMessage publish should succeed (noop on the bus)
        assert!(guard.publish(&NullMessage).is_ok());
    }

    #[test]
    fn test_leader_guard_subscribe() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        // Subscribe with empty topics (all messages)
        let sub = guard.subscribe(&[]);
        assert!(sub.is_ok());
    }

    #[test]
    fn test_leader_guard_subscribe_with_topics() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let sub = guard.subscribe(&[b"topic1", b"topic2"]);
        assert!(sub.is_ok());
    }

    #[test]
    fn test_leader_guard_bus_addresses() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        let addrs = guard.bus_addresses();
        assert!(addrs.frontend.starts_with("ipc://"));
        assert!(addrs.backend.starts_with("ipc://"));
    }

    #[test]
    fn test_follower_guard_publish_noop() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        // Follower publishes to the bus
        assert!(follower.publish(&NullMessage).is_ok());
    }

    #[test]
    fn test_follower_guard_subscribe() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        let sub = follower.subscribe(&[]);
        assert!(sub.is_ok());
    }

    #[test]
    fn test_follower_guard_lock_path() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        assert_eq!(follower.lock_path(), election.lock_path());
    }

    #[test]
    fn test_follower_guard_socket_path() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        assert_eq!(follower.socket_path(), election.socket_path());
    }

    #[test]
    fn test_election_outcome_publish_as_leader() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let outcome = election.elect().unwrap();
        // Publish through the outcome enum directly
        assert!(outcome.publish(&NullMessage).is_ok());
    }

    #[test]
    fn test_election_outcome_publish_as_follower() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        assert!(matches!(&outcome, ElectionOutcome::Follower(_)));
        assert!(outcome.publish(&NullMessage).is_ok());
    }

    #[test]
    fn test_elect_follower_reads_discovery_file() {
        // When a leader is active, the discovery file should exist
        // and the follower should read it to find the proxy addresses
        let dir = TempDir::new().unwrap();
        let config = ElectionConfig::new().with_base_dir(dir.path());
        let election: LeaderElection = LeaderElection::with_config("/ws", config.clone());

        let _leader = election.elect().unwrap();

        // Discovery file should have been written
        let disc = election.discovery_path();
        assert!(disc.exists());

        // Second election should read the discovery file and create a follower
        let election2: LeaderElection = LeaderElection::with_config("/ws", config);
        let outcome = election2.elect().unwrap();
        assert!(matches!(outcome, ElectionOutcome::Follower(_)));
    }
}
