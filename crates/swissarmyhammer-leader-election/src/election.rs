//! File-lock based leader election
//!
//! Uses file locking (`flock`) to coordinate leader election across multiple processes.
//! The first process to acquire the lock becomes the leader; others become followers.
//! Followers can re-contest the election at any time via `try_promote()`.
//!
//! The OS automatically releases `flock` when a process exits or crashes, so a
//! follower calling `try_promote()` will win the election once the old leader is gone.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::bus::{BusMessage, NullMessage, Publisher, Subscriber};
use crate::discovery::{self, BusAddresses};
use crate::error::{ElectionError, Result};
use crate::lease::{self, Clock, SystemClock};
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

/// Best-effort read of the PID currently recorded in a leader lock file.
///
/// Returns `Some(pid)` when the file exists and contains a parseable PID on
/// its first line. Returns `None` when the file is missing, empty, or its
/// contents cannot be parsed as a `u32` — none of which prevents callers
/// from rendering a useful diagnostic, they just omit the PID detail.
///
/// This is intended for follower processes that want to attribute a write
/// failure (e.g. read-only DB) to the actual leader process. It does *not*
/// participate in election semantics: leadership is determined by the flock,
/// not by what's written in the file.
pub fn peek_leader_pid(lock_path: &Path) -> Option<u32> {
    // The lock file now holds a JSON lease whose `pid` field identifies the
    // leader. Prefer that; fall back to the legacy bare-integer format so a
    // lock file written by an older build still resolves.
    if let Some(lease) = lease::read_lease(lock_path) {
        return Some(lease.pid);
    }
    let content = fs::read_to_string(lock_path).ok()?;
    content.lines().next()?.trim().parse::<u32>().ok()
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
    ///
    /// The workspace root is **canonicalized** (symlinks resolved) before the
    /// election key is derived, so two processes that reach the SAME physical
    /// directory via different string forms — e.g. the macOS `/var` vs
    /// `/private/var` (or `/tmp` vs `/private/tmp`) symlink forms, or any
    /// relative/symlinked path — agree on one lock/socket and elect a single
    /// leader (one rust-analyzer / index per workspace root). This is the one
    /// chokepoint every election consumer flows through, so they all agree.
    ///
    /// Canonicalization is idempotent, so a caller that already canonicalized
    /// its root (e.g. the diagnostics tool's `repo_root()`) derives the same
    /// key as one that passed a raw symlink path. Distinct checkouts (git
    /// worktrees) stay distinct canonical dirs, so they correctly keep
    /// separate leaders.
    ///
    /// If canonicalization fails (e.g. the path does not yet exist), the raw
    /// path is used unchanged — the workspace dir normally exists, and this
    /// never panics.
    pub fn with_config(workspace_root: impl AsRef<Path>, config: ElectionConfig) -> Self {
        let raw = workspace_root.as_ref().to_path_buf();
        let workspace_root = fs::canonicalize(&raw).unwrap_or(raw);
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
            Err(ElectionError::LockHeld) => Ok(ElectionOutcome::Follower(self.build_follower()?)),
            Err(e) => Err(e),
        }
    }

    /// Elect as a follower unconditionally, regardless of lock state.
    ///
    /// Used when policy forbids this process from contending for leadership
    /// (see [`crate::may_contend_for_leadership`]) — e.g. a subagent-spawned
    /// `sah serve` that must never index. It builds the same [`FollowerGuard`]
    /// as the `LockHeld` arm of [`Self::elect`] *without* ever attempting the
    /// flock, so a forbidden process never wins even on an empty workspace where
    /// it would otherwise be the leader. The follower can still ride the
    /// leader's bus and (in principle) re-contest, but the caller that opted out
    /// of contention simply never promotes.
    pub fn elect_as_follower_only(&self) -> Result<ElectionOutcome<M>> {
        Ok(ElectionOutcome::Follower(self.build_follower()?))
    }

    /// Construct a [`FollowerGuard`] for this election, reading the discovery
    /// file for the leader's proxy address. Shared by [`Self::elect`] (lock-held
    /// arm) and [`Self::elect_as_follower_only`].
    fn build_follower(&self) -> Result<FollowerGuard<M>> {
        let disc_path = self.discovery_path();
        // Create context once; reused for the publisher and all future subscribe() calls.
        let ctx = zmq::Context::new();
        let publisher = match discovery::read_discovery(&disc_path)? {
            Some(addrs) => Publisher::connected(&ctx, &addrs.frontend)?,
            None => Publisher::noop(),
        };
        Ok(FollowerGuard {
            lock_path: self.lock_path.clone(),
            socket_path: self.socket_path.clone(),
            discovery_path: disc_path,
            bus_addresses: self.bus_addresses(),
            publisher,
            zmq_ctx: ctx,
        })
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

        // Open the lock file *without* truncating. The file may contain a
        // PID written by the current leader; truncating here would wipe it
        // out from under followers that read it via `peek_leader_pid`.
        // The PID is rewritten only after we win the flock.
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)
            .map_err(ElectionError::LockFileCreation)?;

        // Try to acquire exclusive lock (non-blocking)
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // Write an initial lease into the lock file under a fresh nonce
                // so this leader can heartbeat under it and a candidate can
                // detect staleness/takeover. The flock remains the fast path;
                // the lease is the takeover path.
                let nonce = lease::new_nonce();
                let lease_record = lease::Lease {
                    pid: std::process::id(),
                    nonce,
                    heartbeat_ms: SystemClock.now_millis(),
                };
                if let Err(e) = lease::write_lease_atomic(&self.lock_path, &lease_record) {
                    tracing::debug!(
                        path = %self.lock_path.display(),
                        error = %e,
                        "could not write initial leader lease (non-fatal; flock still held)",
                    );
                }
                self.build_leader_guard(lock_file, nonce, true)
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(ElectionError::LockHeld),
            Err(e) => Err(ElectionError::LockAcquisition(e)),
        }
    }

    /// Build a leader guard via the shared free function, using this
    /// election's own paths and bus addresses.
    fn build_leader_guard(
        &self,
        lock_file: File,
        nonce: u64,
        holds_flock: bool,
    ) -> Result<LeaderGuard<M>> {
        build_leader_guard(
            lock_file,
            &self.lock_path,
            &self.socket_path,
            &self.discovery_path(),
            self.bus_addresses(),
            nonce,
            holds_flock,
        )
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

/// Build the leader-side machinery (socket cleanup, bus proxy, discovery file,
/// publisher) and assemble a [`LeaderGuard`].
///
/// Single source of the proxy/discovery/publisher startup shared by initial
/// election ([`LeaderElection::try_acquire_lock`]), flock re-election
/// ([`FollowerGuard::try_promote`]), and lease takeover
/// ([`FollowerGuard::try_promote_via_lease`]) — the three sites differ only in
/// where the paths/addresses come from and whether they hold the OS flock.
///
/// `holds_flock` records whether the resulting guard actually owns the OS flock;
/// a lease-takeover guard may hold leadership purely by lease while the old
/// leader still holds the flock, in which case it must NOT delete the lock file
/// on drop.
#[allow(clippy::too_many_arguments)]
fn build_leader_guard<M: BusMessage>(
    lock_file: File,
    lock_path: &Path,
    socket_path: &Path,
    discovery_path: &Path,
    bus_addresses: BusAddresses,
    nonce: u64,
    holds_flock: bool,
) -> Result<LeaderGuard<M>> {
    // Clean up any stale socket file from a previous run.
    let _ = fs::remove_file(socket_path);

    // Clean up stale IPC sockets before binding.
    discovery::cleanup_discovery(discovery_path, &bus_addresses);

    let proxy = ProxyHandle::start(&bus_addresses)?;
    discovery::write_discovery(discovery_path, &bus_addresses)?;

    // Create a publisher connected to our own proxy.
    let publisher = Publisher::connected(proxy.zmq_context(), &bus_addresses.frontend)?;

    Ok(LeaderGuard {
        _lock_file: lock_file,
        lock_path: lock_path.to_path_buf(),
        socket_path: socket_path.to_path_buf(),
        discovery_path: discovery_path.to_path_buf(),
        bus_addresses,
        _proxy: proxy,
        publisher,
        nonce,
        holds_flock,
    })
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
    /// The lease nonce this leader heartbeats under (identifies its term).
    nonce: u64,
    /// Whether this guard actually owns the OS flock. A lease-takeover guard
    /// may hold leadership purely by lease while the old leader still holds the
    /// flock; such a guard must NOT delete the lock file on drop.
    holds_flock: bool,
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

    /// The lease nonce this leader holds the current term under.
    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    /// Refresh this leader's lease heartbeat under its nonce.
    ///
    /// Returns `true` if the lease is still ours (heartbeat refreshed) and
    /// `false` if we were preempted — another process claimed a stale lease and
    /// wrote a new nonce. A `false` return is the single loss signal: the caller
    /// MUST step down (stop writing) so the single-writer invariant holds.
    pub fn heartbeat(&self, clock: &dyn Clock) -> bool {
        lease::heartbeat(&self.lock_path, self.nonce, clock)
    }
}

impl<M: BusMessage> Drop for LeaderGuard<M> {
    fn drop(&mut self) {
        // Clean up discovery file and IPC sockets unconditionally — they belong
        // to this proxy regardless of how leadership was held.
        discovery::cleanup_discovery(&self.discovery_path, &self.bus_addresses);
        let _ = fs::remove_file(&self.socket_path);
        // Only remove the lock file when we hold the OS flock AND the on-disk
        // lease is still ours. After a lease takeover, the OLD leader may hold
        // the flock while a NEW leader owns the lease in the same file; an old
        // leader stepping down must NOT delete the file out from under the new
        // leader's lease (that would make the new leader's next heartbeat read a
        // missing file and wrongly step down). `lease_held_by` returns false
        // when the file is gone or carries a different nonce, so a preempted
        // leader leaves the file (and the winner's lease) intact.
        if self.holds_flock && lease::lease_held_by(&self.lock_path, self.nonce) {
            let _ = fs::remove_file(&self.lock_path);
        }
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
        // Create-or-open the lock file *without* truncating. The previous
        // leader's drop may have deleted it, so we use create(true) to
        // ensure it exists for flock; but if a current leader holds it,
        // truncating here would wipe out the lease the leader wrote.
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)
            .map_err(ElectionError::LockFileCreation)?;

        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // We won the OS flock — but a free flock does NOT entitle us to
                // leadership if a FRESH lease is still held by a live
                // lease-leader. A leader B can hold leadership purely by lease
                // (its guard has holds_flock=false) while the previous
                // flock-holder still owns the flock; when that old flock-holder
                // exits, the flock goes free even though B is alive and
                // heartbeating. Taking over here would stomp B's fresh lease and
                // preempt a healthy leader. So defer to the lease: if a fresh
                // lease exists, release the flock we just grabbed (by dropping
                // `lock_file`) and report no promotion. Do NOT delete the lock
                // file — it holds B's live lease.
                if let Some(existing) = lease::read_lease(&self.lock_path) {
                    if !existing.is_stale(SystemClock.now_millis(), lease::LEASE_TTL) {
                        // Dropping `lock_file` releases the flock on drop.
                        return Ok(None);
                    }
                }
                // No lease, or a stale lease → the flock-free dead-leader path.
                // Write a fresh lease under a new nonce, then bring up the
                // leader-side machinery. This guard holds the flock, so it
                // cleans up the lock file on drop.
                let nonce = self.write_fresh_lease();
                Ok(Some(self.build_leader_guard(lock_file, nonce, true)?))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(ElectionError::LockAcquisition(e)),
        }
    }

    /// Re-contest via the LEASE rather than the flock.
    ///
    /// This is the takeover path used when the flock is held by a LIVE but stale
    /// leader (it stopped heartbeating). It claims the lease under a fresh nonce
    /// ([`lease::try_claim_lease`]); the old leader will discover it lost when
    /// its next [`LeaderGuard::heartbeat`] returns `false` and must step down.
    ///
    /// On success it brings up the same leader-side machinery as
    /// [`Self::try_promote`], but it does NOT require holding the flock: it opens
    /// the lock file and attempts a best-effort flock. If the flock is free
    /// (the old leader has since exited) the guard records `holds_flock = true`;
    /// if the old leader still holds it (WouldBlock) the guard holds leadership
    /// purely by lease (`holds_flock = false`) and must not delete the lock file
    /// on drop.
    ///
    /// Returns `Ok(None)` when the existing lease is still fresh
    /// ([`lease::ClaimOutcome::Lost`]).
    pub fn try_promote_via_lease(&self, clock: &dyn Clock) -> Result<Option<LeaderGuard<M>>> {
        let nonce = match lease::try_claim_lease(&self.lock_path, clock, lease::LEASE_TTL) {
            lease::ClaimOutcome::Lost => return Ok(None),
            lease::ClaimOutcome::Won { nonce } => nonce,
        };

        // We now own the lease. Open the lock file (no truncate — we must not
        // wipe the lease we just wrote) and attempt a best-effort flock.
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)
            .map_err(ElectionError::LockFileCreation)?;

        let holds_flock = match lock_file.try_lock_exclusive() {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => false,
            Err(e) => return Err(ElectionError::LockAcquisition(e)),
        };

        Ok(Some(self.build_leader_guard(
            lock_file,
            nonce,
            holds_flock,
        )?))
    }

    /// Write a fresh lease under a new nonce and return that nonce. Used by the
    /// flock-fast-path promotion (`try_promote`) where we already own the flock,
    /// so a write failure is non-fatal (the flock is the source of truth).
    fn write_fresh_lease(&self) -> u64 {
        let nonce = lease::new_nonce();
        let record = lease::Lease {
            pid: std::process::id(),
            nonce,
            heartbeat_ms: SystemClock.now_millis(),
        };
        if let Err(e) = lease::write_lease_atomic(&self.lock_path, &record) {
            tracing::debug!(
                path = %self.lock_path.display(),
                error = %e,
                "could not write fresh lease on promotion (non-fatal; flock held)",
            );
        }
        nonce
    }

    /// Build a leader guard via the shared free function, using this follower's
    /// cached paths and bus addresses.
    fn build_leader_guard(
        &self,
        lock_file: File,
        nonce: u64,
        holds_flock: bool,
    ) -> Result<LeaderGuard<M>> {
        build_leader_guard(
            lock_file,
            &self.lock_path,
            &self.socket_path,
            &self.discovery_path,
            self.bus_addresses.clone(),
            nonce,
            holds_flock,
        )
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

    /// Get the bus addresses (for subscribing to the leader's proxy).
    ///
    /// A follower owns no proxy, but it knows where the leader's proxy binds, so
    /// it can connect a [`Subscriber`](crate::Subscriber) to `backend` (via the
    /// public `open` seam) and receive whatever the leader broadcasts.
    pub fn bus_addresses(&self) -> &BusAddresses {
        &self.bus_addresses
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

    /// Two elections opened against symlink-equivalent forms of the *same*
    /// physical directory must derive the SAME lock_path/socket_path so they
    /// elect a single leader (one rust-analyzer / index per workspace root).
    ///
    /// On macOS the canonical tempdir lives under `/private/var/...` but is
    /// reachable through the `/var/...` symlink; a symlink we create here
    /// reproduces the same split (`/var` vs `/private/var`, `/tmp` vs
    /// `/private/tmp`). Before the fix the raw string forms hashed differently
    /// and elected two leaders.
    #[test]
    fn test_symlink_equivalent_roots_derive_same_lock_path() {
        let real = TempDir::new().unwrap();
        let real_root = real.path();

        // A symlink pointing at the same physical directory.
        let link = real
            .path()
            .parent()
            .unwrap()
            .join(format!("leader-election-symlink-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        #[cfg(unix)]
        std::os::unix::fs::symlink(real_root, &link).unwrap();
        #[cfg(not(unix))]
        std::os::windows::fs::symlink_dir(real_root, &link).unwrap();

        let canonical: LeaderElection = LeaderElection::new(real_root);
        let via_link: LeaderElection = LeaderElection::new(&link);

        assert_eq!(
            canonical.lock_path(),
            via_link.lock_path(),
            "symlink-equivalent roots must share one lock_path"
        );
        assert_eq!(
            canonical.socket_path(),
            via_link.socket_path(),
            "symlink-equivalent roots must share one socket_path"
        );

        // And exactly one of them wins the flock; the other is a follower.
        let first = canonical.elect().unwrap();
        assert!(matches!(first, ElectionOutcome::Leader(_)));
        let second = via_link.elect().unwrap();
        assert!(
            matches!(second, ElectionOutcome::Follower(_)),
            "the second election against the same physical dir must be a follower"
        );

        let _ = fs::remove_file(&link);
    }

    /// Canonicalization must NOT over-collapse: two genuinely-distinct
    /// directories (the git-worktree case — each is a distinct checkout that
    /// needs its own rust-analyzer) keep distinct lock_paths and elect
    /// distinct leaders.
    #[test]
    fn test_distinct_dirs_derive_distinct_lock_paths() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        let election_a: LeaderElection = LeaderElection::new(dir_a.path());
        let election_b: LeaderElection = LeaderElection::new(dir_b.path());

        assert_ne!(
            election_a.lock_path(),
            election_b.lock_path(),
            "distinct directories must not collapse to one lock_path"
        );

        // Both win their own election — two separate leaders.
        let a = election_a.elect().unwrap();
        let b = election_b.elect().unwrap();
        assert!(matches!(a, ElectionOutcome::Leader(_)));
        assert!(matches!(b, ElectionOutcome::Leader(_)));
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

    // --- peek_leader_pid tests ---

    /// After a leader acquires the lock, `peek_leader_pid` on the lock path
    /// returns the current process's PID. This is the happy path used by
    /// followers to attribute write-failure messages to a specific process.
    #[test]
    fn test_peek_leader_pid_returns_current_pid_for_active_leader() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _guard = election.try_become_leader().unwrap();

        let pid = peek_leader_pid(election.lock_path());
        assert_eq!(
            pid,
            Some(std::process::id()),
            "leader should have written its own PID into the lock file"
        );
    }

    /// `peek_leader_pid` returns `None` when the lock file does not exist.
    /// Followers must handle this gracefully because the leader may have
    /// exited and cleaned up the lock between the failure and the diagnostic.
    #[test]
    fn test_peek_leader_pid_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let nonexistent = dir.path().join("nonexistent.lock");
        assert_eq!(peek_leader_pid(&nonexistent), None);
    }

    /// `peek_leader_pid` returns `None` for an empty file. This can happen
    /// if a leader is mid-rotation or if a future on-disk format changes.
    #[test]
    fn test_peek_leader_pid_empty_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.lock");
        fs::write(&path, "").unwrap();
        assert_eq!(peek_leader_pid(&path), None);
    }

    /// `peek_leader_pid` returns `None` when the file contains non-numeric
    /// content. The PID record is best-effort; an unparseable value never
    /// causes a hard error.
    #[test]
    fn test_peek_leader_pid_unparseable_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("garbage.lock");
        fs::write(&path, "not-a-pid\n").unwrap();
        assert_eq!(peek_leader_pid(&path), None);
    }

    /// `peek_leader_pid` reads only the first line. Any trailing content
    /// (e.g. future metadata) is ignored, keeping the format forward-compatible.
    #[test]
    fn test_peek_leader_pid_reads_first_line_only() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("multi.lock");
        fs::write(&path, "12345\nextra-line\n").unwrap();
        assert_eq!(peek_leader_pid(&path), Some(12345));
    }

    /// After a leader drops its guard, the lock file is removed and
    /// `peek_leader_pid` returns `None`. Followers will see no PID rather
    /// than a stale one.
    #[test]
    fn test_peek_leader_pid_after_leader_drops_returns_none() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();
        // Sanity: PID is recorded
        assert!(peek_leader_pid(election.lock_path()).is_some());
        drop(guard);
        // Leader Drop removes the lock file
        assert_eq!(peek_leader_pid(election.lock_path()), None);
    }

    /// After a follower promotes itself to leader, the lock file is
    /// rewritten with the new leader's PID. This is the same process in
    /// the test, so the PID stays the same — but the test exercises the
    /// promotion path that also calls `write_leader_pid`.
    #[test]
    fn test_peek_leader_pid_after_promotion() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // Leader writes PID, then dies
        let leader = election.try_become_leader().unwrap();
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        drop(leader);

        // Follower promotes — the promote path must also write the PID
        let _promoted = follower.try_promote().unwrap().unwrap();
        assert_eq!(
            peek_leader_pid(election.lock_path()),
            Some(std::process::id()),
            "promotion path must record the new leader's PID"
        );
    }

    // --- lease takeover tests ---

    /// A freshly-elected leader holds the lease under a nonce, and that lease is
    /// NOT stale at its own heartbeat time. This is the deterministic stand-in
    /// for "a candidate cannot take over a fresh lease": the lease the leader
    /// wrote is held-by its nonce and not stale, so `try_promote_via_lease`
    /// would return Lost.
    #[test]
    fn test_fresh_leader_lease_is_held_and_not_stale() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();

        let lease = crate::lease::read_lease(election.lock_path())
            .expect("leader must have written a lease");
        assert_eq!(lease.nonce, guard.nonce(), "guard nonce matches the lease");
        assert!(
            crate::lease::lease_held_by(election.lock_path(), guard.nonce()),
            "the leader holds the lease under its nonce"
        );
        assert!(
            !lease.is_stale(lease.heartbeat_ms, crate::lease::LEASE_TTL),
            "a fresh lease is not stale at its own heartbeat time"
        );
    }

    /// `try_promote_via_lease` returns None while the existing lease is fresh.
    /// A TestClock pinned to the lease's own heartbeat time makes "fresh"
    /// deterministic regardless of wall-clock drift.
    #[test]
    fn test_try_promote_via_lease_none_when_fresh() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let _leader = election.try_become_leader().unwrap();

        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };

        let lease = crate::lease::read_lease(election.lock_path()).unwrap();
        // Candidate's clock is at the lease's heartbeat time — fully fresh.
        let clock = crate::lease::TestClock::new(lease.heartbeat_ms);
        let promoted = follower.try_promote_via_lease(&clock).unwrap();
        assert!(
            promoted.is_none(),
            "a fresh lease must not be taken over via lease"
        );
    }

    /// `LeaderGuard::heartbeat` returns true normally, and false once another
    /// process has claimed the lease (simulated by writing a foreign-nonce
    /// lease through `write_lease_atomic`). The false return is the leader's
    /// step-down signal.
    #[test]
    fn test_leader_guard_heartbeat_true_then_false_after_preemption() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());
        let guard = election.try_become_leader().unwrap();

        let clock = crate::lease::SystemClock;
        assert!(
            guard.heartbeat(&clock),
            "heartbeat under our own nonce must succeed"
        );

        // Simulate a takeover: a foreign process writes a new lease nonce.
        let foreign = crate::lease::Lease {
            pid: 4242,
            nonce: guard.nonce().wrapping_add(1),
            heartbeat_ms: clock.now_millis(),
        };
        crate::lease::write_lease_atomic(election.lock_path(), &foreign).unwrap();

        assert!(
            !guard.heartbeat(&clock),
            "after preemption the leader's heartbeat must return false (step down)"
        );
    }

    /// End-to-end takeover: a leader holds the flock, but its lease has gone
    /// stale (it stopped heartbeating). A follower takes over via the LEASE path
    /// even though the old leader still holds the flock, and the old leader's
    /// next heartbeat returns false (it must step down). This is the core
    /// single-writer-preserving invariant of the whole change.
    #[test]
    fn test_try_promote_via_lease_takes_over_stale_live_leader() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // Old leader: holds the flock, wrote a lease.
        let old_leader = election.try_become_leader().unwrap();
        let lease = crate::lease::read_lease(election.lock_path()).unwrap();

        // A follower joins (loses the flock).
        let outcome = election.elect().unwrap();
        let follower = match outcome {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };

        // Time advances past the TTL with no heartbeat from the old leader.
        let clock = crate::lease::TestClock::new(
            lease.heartbeat_ms + crate::lease::LEASE_TTL.as_millis() as u64 + 1,
        );

        // The follower takes over via the lease. The old leader still holds the
        // flock, so this guard holds leadership purely by lease.
        let new_leader = follower
            .try_promote_via_lease(&clock)
            .unwrap()
            .expect("a stale lease under a live flock must be claimable");
        assert_ne!(new_leader.nonce(), old_leader.nonce(), "new term");
        assert!(crate::lease::lease_held_by(
            election.lock_path(),
            new_leader.nonce()
        ));

        // The old leader notices it lost the term on its next heartbeat.
        assert!(
            !old_leader.heartbeat(&clock),
            "the preempted old leader must step down (heartbeat false)"
        );
    }

    /// A preempted old leader stepping down (dropping its guard) must NOT delete
    /// the lock file holding the NEW leader's lease. Otherwise the new leader's
    /// next heartbeat reads a missing file and wrongly steps down too.
    #[test]
    fn test_preempted_leader_drop_preserves_new_leader_lease() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // Old leader holds flock + lease.
        let old_leader = election.try_become_leader().unwrap();
        let lease = crate::lease::read_lease(election.lock_path()).unwrap();

        // Follower joins and, after the TTL elapses with no heartbeat, takes
        // over via the lease (old leader still holds the flock).
        let follower = match election.elect().unwrap() {
            ElectionOutcome::Follower(f) => f,
            _ => panic!("expected follower"),
        };
        let clock = crate::lease::TestClock::new(
            lease.heartbeat_ms + crate::lease::LEASE_TTL.as_millis() as u64 + 1,
        );
        let new_leader = follower
            .try_promote_via_lease(&clock)
            .unwrap()
            .expect("stale lease under a live flock must be claimable");
        let new_nonce = new_leader.nonce();

        // The old leader steps down — drop its guard. Its holds_flock is true,
        // but the lease is no longer its nonce, so it must leave the file alone.
        drop(old_leader);

        assert!(
            crate::lease::lease_held_by(election.lock_path(), new_nonce),
            "new leader's lease must survive the old leader's drop"
        );
        assert!(
            new_leader.heartbeat(&clock),
            "new leader must still be able to heartbeat after old leader drops"
        );
    }

    /// A free flock does NOT entitle a process to leadership when a FRESH lease
    /// is held by a live lease-leader. The flock fast path must defer to the lease.
    #[test]
    fn test_try_promote_flock_free_but_fresh_lease_does_not_preempt() {
        let dir = TempDir::new().unwrap();
        let election: LeaderElection = LeaderElection::new(dir.path());

        // Follower handle that never touches the flock.
        let follower = match election.elect_as_follower_only().unwrap() {
            ElectionOutcome::Follower(f) => f,
            ElectionOutcome::Leader(_) => panic!("elect_as_follower_only must be a follower"),
        };

        // Simulate a live lease-leader B: a FRESH lease in the lock file, with the
        // OS flock FREE (nobody holds it in this test).
        let fresh = crate::lease::Lease {
            pid: 4242,
            nonce: 7777,
            heartbeat_ms: crate::lease::SystemClock.now_millis(),
        };
        crate::lease::write_lease_atomic(election.lock_path(), &fresh).unwrap();

        // try_promote can win the FREE flock, but must DEFER to B's fresh lease.
        let promoted = follower.try_promote().unwrap();
        assert!(
            promoted.is_none(),
            "flock-free + fresh-lease must NOT preempt the live lease-leader"
        );
        // B's lease is intact and unchanged.
        assert!(crate::lease::lease_held_by(election.lock_path(), 7777));
    }
}
