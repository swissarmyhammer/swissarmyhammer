//! Code context workspace lifecycle management

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use swissarmyhammer_leader_election::{ElectionConfig, ElectionError, LeaderElection, LeaderGuard};

use crate::db;
use crate::error::CodeContextError;
use crate::indexing::{spawn_indexing_worker, IndexingConfig};

/// Directory name for the code context index
const CONTEXT_DIR: &str = ".code-context";
/// Database filename
const DB_NAME: &str = "index.db";

/// The mode this workspace is operating in
pub enum WorkspaceMode {
    /// Leader: owns the lock, writes to the DB, runs indexers
    Leader {
        /// The database connection (read-write)
        db: Connection,
        /// Guard that holds the leader lock; dropped when the workspace is dropped
        _guard: LeaderGuard,
    },
    /// Reader: queries the DB read-only
    Reader {
        /// The database connection (read-only)
        db: Connection,
    },
}

impl fmt::Debug for WorkspaceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceMode::Leader { .. } => f.write_str("Leader"),
            WorkspaceMode::Reader { .. } => f.write_str("Reader"),
        }
    }
}

/// Manages the `.code-context/` directory and database lifecycle
pub struct CodeContextWorkspace {
    /// The mode (leader or reader)
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
    /// - Attempts leader election
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

        match election.try_become_leader() {
            Ok(guard) => {
                tracing::info!(
                    "Becoming code-context leader for {}",
                    workspace_root.display()
                );

                let db = Connection::open(&db_path)?;
                db::configure_connection(&db)?;
                db::create_schema(&db)?;

                // Populate indexed_files table by scanning the workspace
                // This must happen before spawning the indexing worker so it has files to process
                crate::startup_cleanup(&db, workspace_root)?;

                // Spawn background indexing worker thread in the leader process
                // The worker runs in parallel and updates indexed flags as it completes files
                spawn_indexing_worker(
                    workspace_root.to_path_buf(),
                    db_path,
                    IndexingConfig::default(),
                );

                Ok(Self {
                    mode: WorkspaceMode::Leader { db, _guard: guard },
                    workspace_root: workspace_root.to_path_buf(),
                    context_dir,
                })
            }
            Err(ElectionError::LockHeld) => {
                tracing::info!(
                    "Joining as code-context reader for {}",
                    workspace_root.display()
                );

                let db = Connection::open_with_flags(
                    &db_path,
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                        | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                db::configure_connection(&db)?;

                Ok(Self {
                    mode: WorkspaceMode::Reader { db },
                    workspace_root: workspace_root.to_path_buf(),
                    context_dir,
                })
            }
            Err(e) => Err(CodeContextError::Election(e)),
        }
    }

    /// Whether this workspace is the leader
    pub fn is_leader(&self) -> bool {
        matches!(self.mode, WorkspaceMode::Leader { .. })
    }

    /// Get a reference to the database connection
    pub fn db(&self) -> &Connection {
        match &self.mode {
            WorkspaceMode::Leader { db, .. } => db,
            WorkspaceMode::Reader { db } => db,
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
}
