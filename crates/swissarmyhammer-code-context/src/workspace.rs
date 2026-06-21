//! Code context workspace lifecycle management

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use swissarmyhammer_leader_election::{
    may_contend_for_leadership, ElectionConfig, ElectionOutcome, FollowerGuard, LeaderElection,
    LeaderGuard, SystemClock,
};

use crate::db;
use crate::error::CodeContextError;

/// Directory name for the code context index
const CONTEXT_DIR: &str = ".code-context";
/// Database filename
const DB_NAME: &str = "index.db";

/// Shared write connection to the code-context database.
///
/// SQLite allows exactly one writer at a time. The leader creates a single
/// read-write connection and wraps it in `Arc<Mutex<>>`. All writers
/// (TS indexer, LSP worker, file watcher) share this handle. Readers open
/// their own read-only connections — WAL mode lets them read concurrently
/// without blocking the writer.
pub type SharedDb = Arc<Mutex<Connection>>;

/// The mode this workspace is operating in
pub enum WorkspaceMode {
    /// Leader: owns the lock, writes to the DB, runs indexers
    Leader {
        /// The single shared write connection
        db: SharedDb,
        /// Guard that holds the leader lock; dropped when the workspace is dropped
        _guard: LeaderGuard,
        /// Single-writer enforcement mechanism for this leadership tenure.
        ///
        /// Every worker this leader spawns (the tree-sitter indexer, the file
        /// watcher, the periodic-reconcile loop, and the LSP indexing workers)
        /// is handed a clone of this same `Arc<AtomicBool>` and checks it each
        /// loop iteration. Setting it to `true` — on
        /// [`CodeContextWorkspace::step_down`] or when the workspace is dropped
        /// — tells those workers to stop writing, so the old leader releases the
        /// DB before the new leader becomes the sole writer.
        shutdown: crate::ShutdownFlag,
    },
    /// Follower: queries the DB read-only, can re-contest the election
    Follower {
        /// A read-only database connection
        db: Connection,
        /// Guard that can attempt promotion to leader
        follower: FollowerGuard,
    },
}

impl fmt::Debug for WorkspaceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceMode::Leader { .. } => f.write_str("Leader"),
            WorkspaceMode::Follower { .. } => f.write_str("Follower"),
        }
    }
}

/// The leader-election file paths a [`CodeContextWorkspace`] caches.
///
/// Bundles the lock and request-socket paths so the two role constructors take
/// one argument instead of two; both are read off the [`LeaderElection`] before
/// the election runs and outlive the guard.
struct ElectionPaths {
    /// The leader-election lock file (peeked for the leader PID).
    lock_path: PathBuf,
    /// The leader-election request socket the leader binds / a follower dials.
    socket_path: PathBuf,
}

/// Manages the `.code-context/` directory and database lifecycle
pub struct CodeContextWorkspace {
    /// The mode (leader or follower)
    mode: WorkspaceMode,
    /// Root of the workspace (parent of `.code-context/`)
    workspace_root: PathBuf,
    /// Cached path to the `.code-context/` directory
    context_dir: PathBuf,
    /// Path to the leader-election lock file (used by followers to identify
    /// the leader PID for diagnostic messages).
    lock_path: PathBuf,
    /// Path to the leader-election request socket. The leader binds a
    /// `RequestServer` here and serves the SAH request API; a follower connects
    /// a `SessionRequestClient` to it to route diagnose/LSP queries to the
    /// leader. Surfaced (alongside [`Self::lock_path`]) so the tool layer can
    /// build that client without re-deriving the election paths.
    socket_path: PathBuf,
}

impl fmt::Debug for CodeContextWorkspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeContextWorkspace")
            .field("workspace_root", &self.workspace_root)
            .field("mode", &self.mode)
            .finish()
    }
}

impl CodeContextWorkspace {
    /// Open or create a code context workspace.
    ///
    /// - Creates `.code-context/` if it doesn't exist
    /// - Writes `.gitignore` with `*` to exclude from version control
    /// - Runs leader election (winner writes, followers read)
    /// - Opens the database in WAL mode
    /// - Creates the schema if leader
    pub fn open(workspace_root: &Path) -> Result<Self, CodeContextError> {
        let context_dir = workspace_root.join(CONTEXT_DIR);

        // Ensure directory exists
        fs::create_dir_all(&context_dir)?;

        // Write .gitignore
        let gitignore_path = context_dir.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, "*\n")?;
        }

        // Leader election
        let election = Self::build_election(workspace_root);
        let lock_path = election.lock_path().to_path_buf();
        let socket_path = election.socket_path().to_path_buf();

        let db_path = context_dir.join(DB_NAME);

        let paths = ElectionPaths {
            lock_path,
            socket_path,
        };

        // Subagent-gating policy seam: a process forbidden from contending for
        // leadership (e.g. a subagent-spawned `sah serve`) must NEVER index, so
        // it is elected follower-only — it never wins the flock even on an empty
        // workspace. Default policy permits contention (see
        // `may_contend_for_leadership`).
        let outcome = if may_contend_for_leadership() {
            election.elect()
        } else {
            tracing::info!(
                "code-context: leadership contention disabled for this process \
                 (SAH_SUBAGENT); joining as follower for {}",
                workspace_root.display()
            );
            election.elect_as_follower_only()
        }
        .map_err(CodeContextError::Election)?;

        match outcome {
            ElectionOutcome::Leader(guard) => {
                Self::open_as_leader(workspace_root, context_dir, paths, &db_path, guard)
            }
            ElectionOutcome::Follower(follower) => {
                Self::open_as_follower(workspace_root, context_dir, paths, &db_path, follower)
            }
        }
    }

    /// Construct the [`LeaderElection`] for a workspace. Single source of the
    /// election config (the `code-context` prefix) so `open` and `step_down`
    /// derive the same lock/socket paths.
    fn build_election(workspace_root: &Path) -> LeaderElection {
        let election_config = ElectionConfig::new().with_prefix("code-context");
        LeaderElection::with_config(workspace_root, election_config)
    }

    /// Initialize a leader workspace: create the database, run schema
    /// migrations, and populate the indexed-files table.
    fn open_as_leader(
        workspace_root: &Path,
        context_dir: PathBuf,
        paths: ElectionPaths,
        db_path: &Path,
        guard: LeaderGuard,
    ) -> Result<Self, CodeContextError> {
        tracing::info!(
            "Becoming code-context leader for {}",
            workspace_root.display()
        );

        let conn = Connection::open(db_path)?;
        db::configure_connection(&conn)?;
        db::create_schema(&conn)?;

        // Populate indexed_files table by scanning the workspace.
        // This must happen before spawning the indexing worker so it has files to process.
        crate::startup_cleanup(&conn, workspace_root)?;

        let db = Arc::new(Mutex::new(conn));

        Ok(Self {
            mode: WorkspaceMode::Leader {
                db,
                _guard: guard,
                shutdown: crate::new_shutdown_flag(),
            },
            workspace_root: workspace_root.to_path_buf(),
            context_dir,
            lock_path: paths.lock_path,
            socket_path: paths.socket_path,
        })
    }

    /// Initialize a follower workspace: wait for the leader to create the
    /// database file, then open a read-only connection.
    ///
    /// On the very first run the file may not exist yet. SQLite read-only
    /// open does not create the file, so we retry with a short backoff
    /// (up to ~5 seconds) until it appears.
    ///
    /// Before opening the long-lived read-only connection, the follower
    /// attempts to bring any legacy on-disk schema up to the current version
    /// via a short-lived read-write connection. The migration is idempotent
    /// (it ALTERs only when a column is missing). This protects readers from
    /// a leader that ran older code and left a pre-migration schema on disk.
    /// If the read-write open fails (read-only filesystem, locked, etc.) the
    /// follower logs the condition and continues — the read-only connection
    /// can still serve any query that doesn't touch the new column, and
    /// queries that do hit the missing column will surface a normal SQL
    /// error to the caller.
    fn open_as_follower(
        workspace_root: &Path,
        context_dir: PathBuf,
        paths: ElectionPaths,
        db_path: &Path,
        follower: FollowerGuard,
    ) -> Result<Self, CodeContextError> {
        tracing::debug!(
            "Joining as code-context follower for {}",
            workspace_root.display()
        );

        // Wait for the leader to create the file before doing anything else.
        // open_readonly_with_retry handles the "file may not exist yet" case.
        let db = open_readonly_with_retry(db_path)?;
        db::configure_connection(&db)?;

        // Best-effort: bring a legacy on-disk schema up to current. This is
        // a no-op on a fresh DB created by the current leader.
        migrate_legacy_schema_if_writable(db_path);

        Ok(Self {
            mode: WorkspaceMode::Follower { db, follower },
            workspace_root: workspace_root.to_path_buf(),
            context_dir,
            lock_path: paths.lock_path,
            socket_path: paths.socket_path,
        })
    }

    /// Whether this workspace is the leader
    pub fn is_leader(&self) -> bool {
        matches!(self.mode, WorkspaceMode::Leader { .. })
    }

    /// Get a reference to the database connection.
    ///
    /// For leaders, locks the shared mutex. For followers, returns the
    /// read-only connection directly. Callers should not hold the
    /// returned reference across await points.
    pub fn db(&self) -> DbRef<'_> {
        match &self.mode {
            WorkspaceMode::Leader { db, .. } => {
                DbRef::Shared(db.lock().expect("workspace db mutex poisoned"))
            }
            WorkspaceMode::Follower { db, .. } => DbRef::Owned(db),
        }
    }

    /// Get a reference to the database connection for *write* operations.
    ///
    /// For leaders this behaves identically to [`Self::db`] — the returned
    /// `DbRef::Shared` wraps the leader's read-write connection. For
    /// followers this returns [`CodeContextError::ReadOnlyFollower`] with a
    /// diagnostic identifying the workspace, DB path, and (best-effort) the
    /// leader's PID, so the caller can surface a user-facing explanation
    /// instead of an opaque SQLite "attempt to write a readonly database".
    ///
    /// Write-side ops (currently `rebuild_index`, `clear_status`) should
    /// call this; read-side ops continue to use [`Self::db`].
    pub fn write_db(&self) -> Result<DbRef<'_>, CodeContextError> {
        match &self.mode {
            WorkspaceMode::Leader { db, .. } => Ok(DbRef::Shared(
                db.lock().expect("workspace db mutex poisoned"),
            )),
            WorkspaceMode::Follower { .. } => Err(CodeContextError::ReadOnlyFollower {
                leader_pid: swissarmyhammer_leader_election::peek_leader_pid(&self.lock_path),
                workspace_root: self.workspace_root.clone(),
                db_path: self.context_dir.join(DB_NAME),
            }),
        }
    }

    /// Get the shared write connection handle (leader only).
    ///
    /// Returns `None` for follower workspaces. Workers use this to get
    /// their own clone of the `Arc<Mutex<Connection>>` so they can
    /// write to the database through the single shared connection.
    pub fn shared_db(&self) -> Option<SharedDb> {
        match &self.mode {
            WorkspaceMode::Leader { db, .. } => Some(Arc::clone(db)),
            WorkspaceMode::Follower { .. } => None,
        }
    }

    /// The shutdown flag for the current leadership tenure (leader only).
    ///
    /// Returns `Some(clone)` for a leader and `None` for a follower. The tools
    /// layer hands this SAME flag to every worker it spawns (TS indexer, file
    /// watcher, periodic reconcile, LSP indexing workers); [`Self::step_down`]
    /// sets it, which is how the old leader's writers are stopped before this
    /// process resumes as a follower / before the new leader is the sole writer.
    pub fn leader_shutdown_flag(&self) -> Option<crate::ShutdownFlag> {
        match &self.mode {
            WorkspaceMode::Leader { shutdown, .. } => Some(std::sync::Arc::clone(shutdown)),
            WorkspaceMode::Follower { .. } => None,
        }
    }

    /// The bus addresses for riding the leader's existing pub/sub proxy.
    ///
    /// Returns the leader's XPUB/XSUB proxy addresses for **both** roles: a
    /// leader (which owns the proxy) and a follower (which knows where the
    /// leader's proxy binds). A consumer builds a typed
    /// [`Publisher`](swissarmyhammer_leader_election::Publisher) (leader,
    /// connect to `frontend`) or [`Subscriber`](swissarmyhammer_leader_election::Subscriber)
    /// (follower, connect to `backend`) over the **same** proxy via their public
    /// `open` constructors, rather than starting a second proxy — so the
    /// diagnostics fan-out a leader publishes is exactly what a follower
    /// subscribes to.
    pub fn bus_addresses(
        &self,
    ) -> Option<swissarmyhammer_leader_election::discovery::BusAddresses> {
        match &self.mode {
            WorkspaceMode::Leader { _guard, .. } => Some(_guard.bus_addresses().clone()),
            WorkspaceMode::Follower { follower, .. } => Some(follower.bus_addresses().clone()),
        }
    }

    /// Re-contest the election. Call this periodically on follower workspaces.
    ///
    /// If the current leader has exited (flock released), this process takes
    /// over: opens a read-write connection, runs startup cleanup, and
    /// transitions to leader mode. Returns `Ok(Some(shared_db))` with the
    /// new write connection so callers can start indexing workers.
    ///
    /// Returns `Ok(None)` if already leader or if another process still holds the lock.
    pub fn try_promote(&mut self) -> Result<Option<SharedDb>, CodeContextError> {
        let follower = match &self.mode {
            WorkspaceMode::Leader { .. } => return Ok(None),
            WorkspaceMode::Follower { follower, .. } => follower,
        };

        // First try the flock fast path (the old leader exited, releasing its
        // flock). If that finds the flock still held, fall back to the LEASE
        // takeover path: the holder may be alive but wedged (stopped
        // heartbeating), in which case a stale lease lets us take over anyway.
        let guard = match follower.try_promote().map_err(CodeContextError::Election)? {
            Some(guard) => guard,
            None => match follower
                .try_promote_via_lease(&SystemClock)
                .map_err(CodeContextError::Election)?
            {
                Some(guard) => guard,
                None => return Ok(None),
            },
        };

        Ok(Some(self.become_leader_with_guard(guard)?))
    }

    /// Complete the transition into [`WorkspaceMode::Leader`] given a freshly
    /// won [`LeaderGuard`]: open a read-write connection, run schema migrations
    /// and startup cleanup, install the leader mode, and return the shared write
    /// handle. Shared by both the flock and lease promotion paths.
    fn become_leader_with_guard(
        &mut self,
        guard: LeaderGuard,
    ) -> Result<SharedDb, CodeContextError> {
        tracing::info!(
            "Promoted to code-context leader for {}",
            self.workspace_root.display()
        );

        // Open a read-write connection (the old read-only one is dropped with the mode)
        let db_path = self.context_dir.join(DB_NAME);
        let conn = Connection::open(&db_path)?;
        db::configure_connection(&conn)?;
        db::create_schema(&conn)?;
        crate::startup_cleanup(&conn, &self.workspace_root)?;

        let new_db = Arc::new(Mutex::new(conn));
        let shared = Arc::clone(&new_db);

        self.mode = WorkspaceMode::Leader {
            db: new_db,
            _guard: guard,
            shutdown: crate::new_shutdown_flag(),
        };

        Ok(shared)
    }

    /// Refresh the leader's lease heartbeat (production [`SystemClock`]).
    ///
    /// Returns `true` while this process is still the leader (heartbeat
    /// refreshed under our nonce) and `false` if we were preempted — another
    /// process claimed our stale lease — or if we are a follower. A `false`
    /// return on a leader is the signal to [`Self::step_down`].
    pub fn heartbeat_lease(&self) -> bool {
        match &self.mode {
            WorkspaceMode::Leader { _guard, .. } => _guard.heartbeat(&SystemClock),
            WorkspaceMode::Follower { .. } => false,
        }
    }

    /// Step down from leadership back to follower mode.
    ///
    /// Drops the [`LeaderGuard`] (releasing the flock if we held it) and re-runs
    /// the election as a follower. Because the new leader now holds the lease,
    /// this process lands in [`WorkspaceMode::Follower`] with a fresh read-only
    /// connection and a [`FollowerGuard`] that can later re-contest. No-op when
    /// already a follower.
    ///
    /// Stepping down is how the single-writer invariant is preserved after a
    /// preemption: a leader whose [`Self::heartbeat_lease`] returned `false`
    /// must stop being a writer before the new leader starts writing.
    pub fn step_down(&mut self) {
        if !self.is_leader() {
            return;
        }

        // Enforce single-writer: signal the leader's workers to stop BEFORE we
        // drop the leader guard / read-write connection. The workers each hold
        // their own Arc to this AtomicBool and exit at their next loop check, so
        // the old leader stops writing before this process resumes as a follower
        // and before the new leader becomes the sole writer.
        if let WorkspaceMode::Leader { shutdown, .. } = &self.mode {
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        // Build a follower outcome FIRST (this does not touch the lock), then
        // swap it into `mode` — dropping the old leader guard, which releases
        // the flock (if held) and the read-write DB connection.
        let election = Self::build_election(&self.workspace_root);
        let outcome = election.elect_as_follower_only();
        let db_path = self.context_dir.join(DB_NAME);

        match outcome {
            Ok(ElectionOutcome::Follower(follower)) => match open_readonly_with_retry(&db_path) {
                Ok(db) => {
                    let _ = db::configure_connection(&db);
                    self.mode = WorkspaceMode::Follower { db, follower };
                    tracing::warn!(
                        "code-context: stepped down to follower for {}",
                        self.workspace_root.display()
                    );
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "code-context: step_down could not re-open read-only DB; \
                         using an in-memory placeholder until re-election"
                    );
                    self.mode = WorkspaceMode::Follower {
                        db: Connection::open_in_memory()
                            .expect("in-memory sqlite open cannot fail"),
                        follower,
                    };
                }
            },
            Ok(ElectionOutcome::Leader(_)) => {
                tracing::error!("code-context: step_down unexpectedly elected leader; ignoring");
            }
            Err(e) => {
                tracing::error!(error = %e, "code-context: step_down re-election failed");
            }
        }
    }

    /// Root of the workspace (parent of `.code-context/`)
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Path to the `.code-context/` directory
    pub fn context_dir(&self) -> &Path {
        &self.context_dir
    }

    /// Path to the leader-election request socket.
    ///
    /// The leader binds a request server here (serving the SAH request API onto
    /// its single LSP session); a follower dials this socket to route a
    /// `diagnose` / LSP query to the leader. Paired with [`Self::lock_path`] to
    /// construct the follower-side client.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Path to the leader-election lock file.
    ///
    /// Peeked (via [`peek_leader_pid`](swissarmyhammer_leader_election::peek_leader_pid))
    /// to attribute a "no leader bound" connect failure to the leader PID.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for CodeContextWorkspace {
    /// Stop the leader's writers when the workspace is dropped wholesale.
    ///
    /// Setting the shutdown flag before the connection `Arc` is dropped lets the
    /// workers — which each hold their own `Arc` to the `AtomicBool` — observe it
    /// and exit cleanly, preserving the single-writer invariant even when the
    /// workspace is torn down rather than stepped down.
    fn drop(&mut self) {
        if let WorkspaceMode::Leader { shutdown, .. } = &self.mode {
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Best-effort attempt to bring a legacy on-disk schema up to the current
/// version when this process is a follower.
///
/// Opens a short-lived read-write connection to `db_path` and runs the
/// idempotent additive migrations. Closes the connection before returning.
/// The migration is the same one a fresh leader runs at the end of
/// `create_schema`, so leaders and followers converge on identical schemas.
///
/// All failures (read-only filesystem, file currently locked exclusively
/// elsewhere, migration SQL error) are logged at WARN level and swallowed —
/// the follower's long-lived read-only connection has already been opened,
/// and any caller query against a still-missing column will surface a normal
/// SQL error. This keeps follower startup robust in the face of unusual
/// filesystem permissions while preserving the migration in the common case.
fn migrate_legacy_schema_if_writable(db_path: &Path) {
    let rw_conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                path = %db_path.display(),
                error = %e,
                "follower could not open DB read-write for legacy migration; skipping",
            );
            return;
        }
    };
    if let Err(e) = db::configure_connection(&rw_conn) {
        tracing::warn!(
            path = %db_path.display(),
            error = %e,
            "follower could not configure DB for legacy migration; skipping",
        );
        return;
    }
    if let Err(e) = db::migrate_indexed_files(&rw_conn) {
        tracing::warn!(
            path = %db_path.display(),
            error = %e,
            "follower legacy-schema migration failed; continuing with possibly stale schema",
        );
    }
}

/// Open a read-only SQLite connection, retrying with backoff until the file
/// appears. Gives the leader up to ~5 seconds to create the database.
fn open_readonly_with_retry(db_path: &Path) -> Result<Connection, CodeContextError> {
    let flags =
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut attempts = 0u32;

    loop {
        match Connection::open_with_flags(db_path, flags) {
            Ok(conn) => return Ok(conn),
            Err(e) if attempts < 10 => {
                tracing::debug!(
                    attempt = attempts + 1,
                    path = %db_path.display(),
                    error = %e,
                    "follower waiting for leader to create DB file",
                );
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// A reference to a database connection — either a mutex guard (leader)
/// or a direct reference (reader).
pub enum DbRef<'a> {
    Shared(std::sync::MutexGuard<'a, Connection>),
    Owned(&'a Connection),
}

impl std::fmt::Debug for DbRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbRef::Shared(_) => f.debug_struct("DbRef::Shared").finish_non_exhaustive(),
            DbRef::Owned(_) => f.debug_struct("DbRef::Owned").finish_non_exhaustive(),
        }
    }
}

impl std::ops::Deref for DbRef<'_> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        match self {
            DbRef::Shared(guard) => guard,
            DbRef::Owned(conn) => conn,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_creates_directory_and_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();

        assert!(ws.is_leader());
        assert!(ws.context_dir().exists());

        let gitignore = fs::read_to_string(ws.context_dir().join(".gitignore")).unwrap();
        assert_eq!(gitignore, "*\n");
    }

    #[test]
    fn test_socket_and_lock_paths_are_exposed() {
        // The tool layer constructs a SessionRequestClient from these two paths
        // (the election socket the leader binds, and the lock file the follower
        // peeks for the leader PID), so the workspace must surface both.
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();

        // Both paths live under the same election prefix and are siblings.
        let socket = ws.socket_path();
        let lock = ws.lock_path();
        assert_eq!(socket.parent(), lock.parent());
        assert!(
            socket.extension().is_some_and(|e| e == "sock"),
            "socket path should be a .sock: {}",
            socket.display()
        );
        // The lock path matches the one the workspace already uses for the
        // follower read-only diagnostic.
        assert_eq!(lock, ws.lock_path());
    }

    #[test]
    fn test_open_creates_schema() {
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();

        let tables: Vec<String> = ws
            .db()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(tables.contains(&"indexed_files".to_string()));
        assert!(tables.contains(&"ts_chunks".to_string()));
        assert!(tables.contains(&"lsp_symbols".to_string()));
        assert!(tables.contains(&"lsp_call_edges".to_string()));
    }

    #[test]
    fn test_second_open_is_reader() {
        let dir = tempfile::tempdir().unwrap();

        // First open becomes leader
        let ws1 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws1.is_leader());

        // Second open becomes reader
        let ws2 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!ws2.is_leader());
    }

    #[test]
    fn test_gitignore_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let context_dir = dir.path().join(".code-context");
        fs::create_dir_all(&context_dir).unwrap();
        fs::write(context_dir.join(".gitignore"), "custom\n").unwrap();

        let _ws = CodeContextWorkspace::open(dir.path()).unwrap();

        let content = fs::read_to_string(context_dir.join(".gitignore")).unwrap();
        assert_eq!(content, "custom\n");
    }

    #[test]
    fn test_workspace_root_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();

        assert_eq!(ws.workspace_root(), dir.path());
    }

    #[test]
    fn test_debug_impls() {
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();

        let debug_str = format!("{:?}", ws);
        assert!(debug_str.contains("CodeContextWorkspace"));
        assert!(debug_str.contains("Leader"));
    }

    #[test]
    fn test_shared_db_leader_returns_some() {
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws.is_leader());
        assert!(ws.shared_db().is_some());
    }

    #[test]
    fn test_shared_db_follower_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let ws1 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws1.is_leader());

        let ws2 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!ws2.is_leader());
        assert!(ws2.shared_db().is_none());
    }

    #[test]
    fn test_try_promote_noop_for_leader() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws.is_leader());

        // Promoting a leader is a no-op
        let result = ws.try_promote().unwrap();
        assert!(result.is_none());
        assert!(ws.is_leader());
    }

    #[test]
    fn test_try_promote_fails_while_leader_alive() {
        let dir = tempfile::tempdir().unwrap();
        let _ws1 = CodeContextWorkspace::open(dir.path()).unwrap();

        let mut ws2 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!ws2.is_leader());

        // Leader still alive — promotion should fail
        let result = ws2.try_promote().unwrap();
        assert!(result.is_none());
        assert!(!ws2.is_leader());
    }

    #[test]
    fn test_try_promote_succeeds_after_leader_drops() {
        let dir = tempfile::tempdir().unwrap();
        let ws1 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws1.is_leader());

        let mut ws2 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!ws2.is_leader());

        // Leader exits
        drop(ws1);

        // Follower promotes
        let shared_db = ws2.try_promote().unwrap();
        assert!(shared_db.is_some());
        assert!(ws2.is_leader());
        assert!(ws2.shared_db().is_some());
    }

    #[test]
    fn test_promoted_workspace_blocks_others() {
        let dir = tempfile::tempdir().unwrap();
        let ws1 = CodeContextWorkspace::open(dir.path()).unwrap();

        let mut ws2 = CodeContextWorkspace::open(dir.path()).unwrap();
        drop(ws1);

        // ws2 promotes
        let _db = ws2.try_promote().unwrap().unwrap();
        assert!(ws2.is_leader());

        // ws3 should be a follower
        let ws3 = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!ws3.is_leader());
    }

    #[test]
    fn test_follower_db_returns_owned_dbref_that_can_query() {
        let dir = tempfile::tempdir().unwrap();
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();

        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        // Follower's db() returns DbRef::Owned which derefs to &Connection.
        // Verify it can execute read queries through the Deref impl.
        let tables: Vec<String> = follower
            .db()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(tables.contains(&"indexed_files".to_string()));
        assert!(tables.contains(&"ts_chunks".to_string()));
    }

    #[test]
    fn test_follower_reads_leader_data() {
        let dir = tempfile::tempdir().unwrap();
        let leader = CodeContextWorkspace::open(dir.path()).unwrap();

        // Leader writes a row
        leader
            .db()
            .execute(
                "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
                 VALUES ('test.rs', X'AABB', 42, 9999)",
                [],
            )
            .unwrap();

        // Follower opens and reads through its DbRef::Owned path
        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        let count: i64 = follower
            .db()
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE file_path = 'test.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_workspace_mode_debug_follower() {
        let dir = tempfile::tempdir().unwrap();
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();

        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        let debug_str = format!("{:?}", follower);
        assert!(
            debug_str.contains("Follower"),
            "expected 'Follower' in debug output, got: {debug_str}"
        );
        assert!(debug_str.contains("CodeContextWorkspace"));
    }

    #[test]
    fn test_dbref_debug_shared() {
        let dir = tempfile::tempdir().unwrap();
        let leader = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(leader.is_leader());

        let db_ref = leader.db();
        let debug_str = format!("{:?}", db_ref);
        assert!(
            debug_str.contains("DbRef::Shared"),
            "expected 'DbRef::Shared' in debug output, got: {debug_str}"
        );
    }

    #[test]
    fn test_dbref_debug_owned() {
        let dir = tempfile::tempdir().unwrap();
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();

        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        let db_ref = follower.db();
        let debug_str = format!("{:?}", db_ref);
        assert!(
            debug_str.contains("DbRef::Owned"),
            "expected 'DbRef::Owned' in debug output, got: {debug_str}"
        );
    }

    #[test]
    fn test_try_promote_shared_db_is_functional() {
        let dir = tempfile::tempdir().unwrap();

        // Write a file so startup_cleanup has something to discover
        fs::write(dir.path().join("hello.rs"), "fn main() {}").unwrap();

        let leader = CodeContextWorkspace::open(dir.path()).unwrap();
        let mut follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());
        drop(leader);

        // Promote and get the shared DB handle
        let shared_db = follower
            .try_promote()
            .unwrap()
            .expect("promotion should succeed");

        // The shared DB should allow writes
        {
            let conn = shared_db.lock().expect("mutex not poisoned");
            conn.execute(
                "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
                 VALUES ('promoted_test.rs', X'CCDD', 100, 5555)",
                [],
            )
            .unwrap();
        }

        // Read it back through the workspace's own db() accessor
        let count: i64 = follower
            .db()
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE file_path = 'promoted_test.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_try_promote_runs_startup_cleanup() {
        let dir = tempfile::tempdir().unwrap();

        // Create a source file before any workspace opens
        fs::write(dir.path().join("lib.rs"), "pub fn init() {}").unwrap();

        let leader = CodeContextWorkspace::open(dir.path()).unwrap();
        let mut follower = CodeContextWorkspace::open(dir.path()).unwrap();
        drop(leader);

        let _shared = follower
            .try_promote()
            .unwrap()
            .expect("promotion should succeed");

        // startup_cleanup discovers workspace files and populates indexed_files.
        // Verify the file was discovered.
        let count: i64 = follower
            .db()
            .query_row("SELECT COUNT(*) FROM indexed_files", [], |r| r.get(0))
            .unwrap();
        assert!(
            count >= 1,
            "startup_cleanup should have discovered at least one file, found {count}"
        );
    }

    #[test]
    fn test_follower_retry_succeeds_when_db_exists() {
        // When a leader has already created the DB, the follower retry loop
        // should succeed on the first attempt (no retries needed).
        // This exercises the Ok(conn) => break path in the retry loop.
        let dir = tempfile::tempdir().unwrap();
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();

        // DB file exists because the leader created it
        let db_path = dir.path().join(".code-context").join("index.db");
        assert!(db_path.exists(), "leader should have created the DB file");

        // Follower open should succeed immediately through the retry loop
        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        // Verify the follower's connection is usable
        let result: String = follower
            .db()
            .query_row("SELECT sqlite_version()", [], |r| r.get(0))
            .unwrap();
        assert!(
            !result.is_empty(),
            "should get a valid SQLite version string"
        );
    }

    /// Helper that constructs a `.code-context/index.db` containing the
    /// pre-`embedded`-column schema. Mirrors the bare CREATE TABLE the
    /// pre-migration code shipped, without ever invoking `create_schema`.
    fn create_legacy_db_in(workspace_root: &Path) -> PathBuf {
        let context_dir = workspace_root.join(CONTEXT_DIR);
        fs::create_dir_all(&context_dir).unwrap();
        let db_path = context_dir.join(DB_NAME);

        let conn = Connection::open(&db_path).unwrap();
        // Use the WAL+foreign keys configuration so the file format matches
        // what production opens later, but skip create_schema so the
        // `embedded` column is intentionally absent.
        db::configure_connection(&conn).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE indexed_files (
                file_path     TEXT PRIMARY KEY,
                content_hash  BLOB NOT NULL,
                file_size     INTEGER NOT NULL,
                last_seen_at  INTEGER NOT NULL,
                ts_indexed    INTEGER NOT NULL DEFAULT 0,
                lsp_indexed   INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE ts_chunks (
                file_path    TEXT NOT NULL REFERENCES indexed_files(file_path) ON DELETE CASCADE,
                start_byte   INTEGER NOT NULL,
                end_byte     INTEGER NOT NULL,
                start_line   INTEGER NOT NULL,
                end_line     INTEGER NOT NULL,
                text         TEXT NOT NULL,
                symbol_path  TEXT,
                embedding    BLOB
            );
            ",
        )
        .unwrap();
        db_path
    }

    /// Read column names from `PRAGMA table_info(indexed_files)`.
    fn read_indexed_files_columns(db_path: &Path) -> Vec<String> {
        let conn = Connection::open(db_path).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(indexed_files)").unwrap();
        stmt.query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// SQLite `ALTER TABLE ... DROP COLUMN` was added in 3.35.0. Use it to
    /// roll back the `embedded` column for tests that need to simulate a
    /// legacy on-disk schema after the leader has already migrated.
    fn drop_embedded_column(db_path: &Path) {
        let conn = Connection::open(db_path).unwrap();
        conn.execute("ALTER TABLE indexed_files DROP COLUMN embedded", [])
            .unwrap();
    }

    /// A follower that joins an existing workspace whose on-disk schema is
    /// missing the `embedded` column must trigger the legacy migration so
    /// later queries against the new column succeed.
    ///
    /// This locks in the fix for the leader-vs-follower migration gap.
    /// `create_schema` only runs on the leader path; if the running leader
    /// were OLD code that did not migrate, the follower-side migration is
    /// the only thing keeping reads against the new column from failing
    /// with `SQLite Error: no such column: embedded`.
    ///
    /// To isolate follower-side migration from leader-side migration, the
    /// test drops the `embedded` column out from under the leader after it
    /// has opened — simulating "leader was running pre-migration code" —
    /// then opens a follower and verifies the column comes back.
    #[test]
    fn test_follower_migrates_legacy_schema_on_open() {
        let dir = tempfile::tempdir().unwrap();

        // Stage a legacy schema on disk before any workspace opens.
        let db_path = create_legacy_db_in(dir.path());
        let pre_cols = read_indexed_files_columns(&db_path);
        assert!(
            !pre_cols.iter().any(|c| c == "embedded"),
            "precondition: legacy schema should not have `embedded` column, got: {:?}",
            pre_cols
        );

        // The first open is leader. Its `create_schema` will migrate, but
        // we are testing the follower path, so we undo that migration on
        // disk to simulate a leader that ran older code.
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();
        drop_embedded_column(&db_path);
        let mid_cols = read_indexed_files_columns(&db_path);
        assert!(
            !mid_cols.iter().any(|c| c == "embedded"),
            "precondition: after dropping the column the legacy state is restored, got: {:?}",
            mid_cols
        );

        // The follower opens against a "legacy" DB. Its open_as_follower
        // path must run the migration through a brief read-write connection.
        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader(), "second open must be a follower");

        let post_cols = read_indexed_files_columns(&db_path);
        assert!(
            post_cols.iter().any(|c| c == "embedded"),
            "expected follower-side migration to add `embedded` column, got: {:?}",
            post_cols
        );

        // And the follower's read-only connection must be able to query it.
        let count: i64 = follower
            .db()
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE embedded = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "no rows yet, but query against `embedded` works");
    }

    /// `migrate_legacy_schema_if_writable` is best-effort: if the read-write
    /// open fails (e.g. the path is bogus) it logs and returns without
    /// panicking. The follower path can still proceed with its existing
    /// read-only connection.
    #[test]
    fn test_migrate_legacy_schema_swallows_open_failure() {
        // A path that cannot be opened — directory does not exist, so SQLite
        // cannot create the file (Connection::open creates the DB file if
        // missing, but only when the parent exists).
        let bogus = Path::new("/nonexistent/parent/dir/doesnotexist.db");
        // Must not panic.
        migrate_legacy_schema_if_writable(bogus);
    }

    /// `migrate_legacy_schema_if_writable` is idempotent: running it twice
    /// against an already-current DB leaves a single `embedded` column.
    #[test]
    fn test_migrate_legacy_schema_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = create_legacy_db_in(dir.path());

        migrate_legacy_schema_if_writable(&db_path);
        migrate_legacy_schema_if_writable(&db_path);

        let cols = read_indexed_files_columns(&db_path);
        let n = cols.iter().filter(|c| *c == "embedded").count();
        assert_eq!(n, 1, "`embedded` column should appear exactly once");
    }

    // --- write_db tests ---

    /// On a leader workspace, `write_db()` returns a usable DbRef that can
    /// execute writes. This mirrors `db()`'s leader behaviour exactly.
    #[test]
    fn test_write_db_leader_returns_ok_and_can_write() {
        let dir = tempfile::tempdir().unwrap();
        let ws = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws.is_leader());

        let db = ws.write_db().expect("leader's write_db must succeed");
        db.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('write_db.rs', X'1234', 1, 1)",
            [],
        )
        .unwrap();
    }

    /// On a follower workspace, `write_db()` returns
    /// `CodeContextError::ReadOnlyFollower` *before* any SQL runs. The
    /// payload identifies the workspace, the DB path, and (best-effort) the
    /// leader's PID — the leader is this same process in the test, so the
    /// PID should match.
    #[test]
    fn test_write_db_follower_returns_typed_error() {
        let dir = tempfile::tempdir().unwrap();

        // Hold the leader on this process so the follower's lock attempt
        // loses the election deterministically.
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();

        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        let err = follower
            .write_db()
            .expect_err("follower's write_db must reject the write");

        match err {
            CodeContextError::ReadOnlyFollower {
                leader_pid,
                workspace_root,
                db_path,
            } => {
                assert_eq!(
                    leader_pid,
                    Some(std::process::id()),
                    "expected the leader's PID (same process in this test)"
                );
                assert_eq!(workspace_root, dir.path());
                assert_eq!(db_path, dir.path().join(CONTEXT_DIR).join(DB_NAME));
            }
            other => panic!("expected ReadOnlyFollower, got: {:?}", other),
        }
    }

    /// The `ReadOnlyFollower` Display message names the workspace and DB
    /// paths so the user can identify the offending leader session. This
    /// is what surfaces in the MCP error payload.
    #[test]
    fn test_write_db_follower_error_message_mentions_workspace_and_db() {
        let dir = tempfile::tempdir().unwrap();
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();
        let follower = CodeContextWorkspace::open(dir.path()).unwrap();

        let err = follower.write_db().expect_err("follower rejects write_db");
        let msg = err.to_string();

        let ws_display = dir.path().display().to_string();
        let db_display = dir
            .path()
            .join(CONTEXT_DIR)
            .join(DB_NAME)
            .display()
            .to_string();
        assert!(
            msg.contains(&ws_display),
            "message should mention workspace root, got: {msg}"
        );
        assert!(
            msg.contains(&db_display),
            "message should mention DB path, got: {msg}"
        );
        let pid_str = format!("pid {}", std::process::id());
        assert!(
            msg.contains(&pid_str),
            "message should mention leader pid, got: {msg}"
        );
    }

    /// After promoting a follower to leader, `write_db()` succeeds — the
    /// workspace transitions to the leader branch.
    #[test]
    fn test_write_db_after_promotion_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let leader = CodeContextWorkspace::open(dir.path()).unwrap();
        let mut follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());

        // Follower rejects writes
        assert!(follower.write_db().is_err());

        drop(leader);
        let _shared = follower.try_promote().unwrap().expect("promotion succeeds");

        // Now write_db works
        let db = follower
            .write_db()
            .expect("promoted workspace must accept writes");
        db.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('post_promote.rs', X'AB', 1, 1)",
            [],
        )
        .unwrap();
    }

    /// A process forbidden from contending (SAH_SUBAGENT=1) must NOT become the
    /// leader even on an empty workspace where it would otherwise win. The env
    /// var is set, the workspace opened, asserted, and the var removed all
    /// inline so this does not leak global env state to parallel tests.
    #[test]
    fn test_subagent_opens_as_follower_only() {
        let dir = tempfile::tempdir().unwrap();
        // A normal (allowed) process is the real leader and creates the DB.
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();

        let prior = std::env::var("SAH_SUBAGENT").ok();
        std::env::set_var("SAH_SUBAGENT", "1");
        let ws = CodeContextWorkspace::open(dir.path());
        // Restore env before asserting so a panic does not leak it.
        match prior {
            Some(v) => std::env::set_var("SAH_SUBAGENT", v),
            None => std::env::remove_var("SAH_SUBAGENT"),
        }

        let ws = ws.expect("follower-only open should still succeed");
        assert!(
            !ws.is_leader(),
            "a SAH_SUBAGENT=1 process must never win leadership"
        );
    }

    /// `heartbeat_lease` returns true for a live leader, and after `step_down`
    /// the workspace is no longer the leader. This exercises the preemption
    /// recovery path: a leader that lost its lease steps down to follower.
    #[test]
    fn test_step_down_transitions_leader_to_follower() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws.is_leader());
        assert!(
            ws.heartbeat_lease(),
            "a live leader heartbeats successfully"
        );

        ws.step_down();
        assert!(
            !ws.is_leader(),
            "after step_down the workspace is a follower"
        );
        assert!(
            !ws.heartbeat_lease(),
            "a follower never reports a successful heartbeat"
        );
    }

    /// `heartbeat_lease` returns false for a follower (it owns no lease).
    #[test]
    fn test_heartbeat_lease_false_for_follower() {
        let dir = tempfile::tempdir().unwrap();
        let _leader = CodeContextWorkspace::open(dir.path()).unwrap();
        let follower = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(!follower.is_leader());
        assert!(!follower.heartbeat_lease());
    }

    #[test]
    fn test_step_down_sets_shutdown_flag() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = CodeContextWorkspace::open(dir.path()).unwrap();
        assert!(ws.is_leader());
        let flag = ws
            .leader_shutdown_flag()
            .expect("leader has a shutdown flag");
        assert!(!flag.load(std::sync::atomic::Ordering::Relaxed));
        ws.step_down();
        assert!(!ws.is_leader());
        assert!(
            flag.load(std::sync::atomic::Ordering::Relaxed),
            "step_down must set the shutdown flag so the old leader's workers stop writing"
        );
    }
}
