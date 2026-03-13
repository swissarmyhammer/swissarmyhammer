//! Code context workspace lifecycle management

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use swissarmyhammer_leader_election::{
    ElectionConfig, ElectionOutcome, FollowerGuard, LeaderElection, LeaderGuard,
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

/// Manages the `.code-context/` directory and database lifecycle
pub struct CodeContextWorkspace {
    /// The mode (leader or follower)
    mode: WorkspaceMode,
    /// Root of the workspace (parent of `.code-context/`)
    workspace_root: PathBuf,
    /// Cached path to the `.code-context/` directory
    context_dir: PathBuf,
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
        let election_config = ElectionConfig::new().with_prefix("code-context");
        let election = LeaderElection::with_config(workspace_root, election_config);

        let db_path = context_dir.join(DB_NAME);

        match election.elect().map_err(CodeContextError::Election)? {
            ElectionOutcome::Leader(guard) => {
                tracing::info!(
                    "Becoming code-context leader for {}",
                    workspace_root.display()
                );

                let conn = Connection::open(&db_path)?;
                db::configure_connection(&conn)?;
                db::create_schema(&conn)?;

                // Populate indexed_files table by scanning the workspace
                // This must happen before spawning the indexing worker so it has files to process
                crate::startup_cleanup(&conn, workspace_root)?;

                let db = Arc::new(Mutex::new(conn));

                Ok(Self {
                    mode: WorkspaceMode::Leader { db, _guard: guard },
                    workspace_root: workspace_root.to_path_buf(),
                    context_dir,
                })
            }
            ElectionOutcome::Follower(follower) => {
                tracing::debug!(
                    "Joining as code-context follower for {}",
                    workspace_root.display()
                );

                // Wait for the leader to create the DB file before opening it
                // read-only. On the very first run the file may not exist yet.
                // SQLite read-only open does not create the file, so we retry
                // with a short backoff (up to ~5 seconds) until it appears.
                let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
                let mut attempts = 0u32;
                let db = loop {
                    match Connection::open_with_flags(&db_path, flags) {
                        Ok(conn) => break conn,
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
                };
                db::configure_connection(&db)?;

                Ok(Self {
                    mode: WorkspaceMode::Follower { db, follower },
                    workspace_root: workspace_root.to_path_buf(),
                    context_dir,
                })
            }
        }
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

        let guard = match follower.try_promote().map_err(CodeContextError::Election)? {
            Some(guard) => guard,
            None => return Ok(None),
        };

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
        };

        Ok(Some(shared))
    }

    /// Root of the workspace (parent of `.code-context/`)
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Path to the `.code-context/` directory
    pub fn context_dir(&self) -> &Path {
        &self.context_dir
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
}
