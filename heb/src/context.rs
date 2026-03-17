//! HebContext — XDG-aware entry point for the Hook Event Bus.
//!
//! Opening a context participates in leader election. Every process that
//! touches heb is a potential leader. Whoever wins starts the proxy.

use std::path::{Path, PathBuf};

use swissarmyhammer_leader_election::{ElectionConfig, ElectionOutcome, LeaderElection};

use crate::error::{HebError, Result};
use crate::event::HebEvent;
use crate::header::EventHeader;
use crate::store;

/// The HEB context — wraps leader election bus with XDG paths and SQLite persistence.
pub struct HebContext {
    /// XDG_DATA_HOME/heb/<workspace_hash>/ — database lives here, scoped to the workspace
    data_dir: PathBuf,
    /// Canonical path of the workspace root this context was opened for
    workspace_root: PathBuf,
    /// The election outcome — leader or follower, both can publish
    election: ElectionOutcome<HebEvent>,
}

impl HebContext {
    /// Open a HEB context for the given workspace.
    ///
    /// This is not passive — it participates in leader election.
    /// No discovery file = no leader = contest the election.
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());
        let data_dir = Self::resolve_data_dir(&canonical);
        let runtime_dir = Self::resolve_runtime_dir();
        Self::open_with_dirs(workspace_root, &data_dir, &runtime_dir, "heb")
    }

    /// Open a HEB context with explicit directory paths and election prefix.
    ///
    /// Shared by `open()` and tests. Canonicalizes `workspace_root`.
    fn open_with_dirs(
        workspace_root: &Path,
        data_dir: &Path,
        runtime_dir: &Path,
        prefix: &str,
    ) -> Result<Self> {
        let canonical = workspace_root
            .canonicalize()
            .unwrap_or_else(|_| workspace_root.to_path_buf());

        // Ensure data dir exists and schema is initialized
        let db_path = data_dir.join("events.db");
        store::init_schema(&db_path)?;

        let config = ElectionConfig::new()
            .with_prefix(prefix)
            .with_base_dir(runtime_dir);

        let election = LeaderElection::<HebEvent>::with_config(workspace_root, config)
            .elect()
            .map_err(HebError::Election)?;

        Ok(HebContext {
            data_dir: data_dir.to_path_buf(),
            workspace_root: canonical,
            election,
        })
    }

    /// Publish: write to SQLite (open/write/close) + send via ZMQ PUB.
    ///
    /// Every publisher persists independently. Most reliable.
    /// Returns the event's ULID.
    pub fn publish(&self, header: &EventHeader, body: &[u8]) -> Result<String> {
        // 1. Always persist — this is the durable path
        let id = store::log_event(&self.db_path(), header, body)?;

        // 2. Always send via ZMQ — this is the live path (best-effort)
        let event = HebEvent {
            header: header.clone(),
            body: body.to_vec(),
        };
        if let Err(e) = self.election.publish(&event) {
            tracing::warn!(id = %id, error = %e, "ZMQ publish failed (event persisted to SQLite)");
        }

        Ok(id)
    }

    /// Replay from SQLite (catch-up after leader transition gap).
    ///
    /// Pass an empty string to replay from the beginning.
    pub fn replay(
        &self,
        since_id: &str,
        category: Option<&str>,
    ) -> Result<Vec<(EventHeader, Vec<u8>)>> {
        store::replay(&self.db_path(), since_id, category)
    }

    /// Get the database path.
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("events.db")
    }

    /// Get the canonical workspace root this context was opened for.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Resolve XDG_DATA_HOME/heb/<workspace_hash>/ (default base: ~/.local/share/heb/)
    ///
    /// The workspace hash is the md5 hex digest of the canonical workspace root path,
    /// ensuring each workspace gets its own isolated SQLite database.
    fn resolve_data_dir(workspace_root: &Path) -> PathBuf {
        let hash = format!(
            "{:x}",
            md5::compute(workspace_root.to_string_lossy().as_bytes())
        );
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".local")
                    .join("share")
            })
            .join("heb")
            .join(hash)
    }

    /// Resolve XDG_RUNTIME_DIR/heb/ (fallback: temp dir)
    fn resolve_runtime_dir() -> PathBuf {
        std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("heb")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::EventCategory;
    use tempfile::TempDir;

    /// Create a HebContext with test-local paths instead of XDG defaults.
    ///
    /// Delegates to `open_with_dirs` so tests exercise the same code path as `open()`.
    fn test_context(workspace: &Path, data_dir: &Path, runtime_dir: &Path) -> Result<HebContext> {
        HebContext::open_with_dirs(workspace, data_dir, runtime_dir, "heb-test")
    }

    #[test]
    fn test_publish_persists_to_sqlite() {
        let dir = TempDir::new().unwrap();
        let workspace = dir.path().join("workspace");
        let data_dir = dir.path().join("data");
        let runtime_dir = dir.path().join("runtime");
        std::fs::create_dir_all(&workspace).unwrap();

        let ctx = test_context(&workspace, &data_dir, &runtime_dir).unwrap();

        let header = EventHeader::new(
            "sess-1",
            "/workspace",
            EventCategory::Hook,
            "pre_tool_use",
            "avp-hook",
        );
        let id = ctx.publish(&header, b"test body").unwrap();
        assert_eq!(id, header.id);

        // Verify it's in SQLite
        let events = ctx.replay("", None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.event_type, "pre_tool_use");
        assert_eq!(events[0].1, b"test body");
    }

    #[test]
    fn test_different_workspace_roots_produce_different_db_paths() {
        let root_a = Path::new("/tmp/workspace_a");
        let root_b = Path::new("/tmp/workspace_b");

        let data_dir_a = HebContext::resolve_data_dir(root_a);
        let data_dir_b = HebContext::resolve_data_dir(root_b);

        // Different workspace roots must produce different data directories
        assert_ne!(
            data_dir_a, data_dir_b,
            "Different workspace roots must map to different data directories"
        );

        // Same workspace root must produce the same data directory (stable hashing)
        let data_dir_a_again = HebContext::resolve_data_dir(root_a);
        assert_eq!(
            data_dir_a, data_dir_a_again,
            "Same workspace root must always produce the same data directory"
        );
    }

    #[test]
    fn test_replay_filtered_by_category() {
        let dir = TempDir::new().unwrap();
        let workspace = dir.path().join("workspace");
        let data_dir = dir.path().join("data");
        let runtime_dir = dir.path().join("runtime");
        std::fs::create_dir_all(&workspace).unwrap();

        let ctx = test_context(&workspace, &data_dir, &runtime_dir).unwrap();

        let h1 = EventHeader::new("s", "/w", EventCategory::Hook, "test", "src");
        let h2 = EventHeader::new("s", "/w", EventCategory::Session, "start", "src");
        ctx.publish(&h1, b"1").unwrap();
        ctx.publish(&h2, b"2").unwrap();

        let hooks = ctx.replay("", Some("hook")).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].0.category, EventCategory::Hook);
    }
}
