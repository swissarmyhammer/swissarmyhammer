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
    /// XDG_DATA_HOME/heb/ — database lives here
    data_dir: PathBuf,
    /// The election outcome — leader or follower, both can publish
    election: ElectionOutcome<HebEvent>,
}

impl HebContext {
    /// Open a HEB context for the given workspace.
    ///
    /// This is not passive — it participates in leader election.
    /// No discovery file = no leader = contest the election.
    pub fn open(workspace_root: &Path) -> Result<Self> {
        let data_dir = Self::resolve_data_dir();
        let runtime_dir = Self::resolve_runtime_dir();

        // Ensure data dir exists and schema is initialized
        let db_path = data_dir.join("events.db");
        store::init_schema(&db_path)?;

        let config = ElectionConfig::new()
            .with_prefix("heb")
            .with_base_dir(&runtime_dir);

        let election = LeaderElection::<HebEvent>::with_config(workspace_root, config)
            .elect()
            .map_err(HebError::Election)?;

        Ok(HebContext {
            data_dir,
            election,
        })
    }

    /// Publish: write to SQLite (open/write/close) + send via ZMQ PUB.
    ///
    /// Every publisher persists independently. Most reliable.
    pub fn publish(&self, header: &EventHeader, body: &[u8]) -> Result<u64> {
        // 1. Always persist — this is the durable path
        let seq = store::log_event(&self.db_path(), header, body)?;

        // 2. Always send via ZMQ — this is the live path (best-effort)
        let event = HebEvent {
            header: header.clone(),
            body: body.to_vec(),
        };
        let _ = self.election.publish(&event);

        Ok(seq)
    }

    /// Replay from SQLite (catch-up after leader transition gap).
    pub fn replay(
        &self,
        since_seq: u64,
        category: Option<&str>,
    ) -> Result<Vec<(EventHeader, Vec<u8>)>> {
        store::replay(&self.db_path(), since_seq, category)
    }

    /// Get the database path.
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("events.db")
    }

    /// Resolve XDG_DATA_HOME/heb/ (default: ~/.local/share/heb/)
    fn resolve_data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".local")
                    .join("share")
            })
            .join("heb")
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
    fn test_context(workspace: &Path, data_dir: &Path, runtime_dir: &Path) -> Result<HebContext> {
        let db_path = data_dir.join("events.db");
        store::init_schema(&db_path)?;

        let config = ElectionConfig::new()
            .with_prefix("heb-test")
            .with_base_dir(runtime_dir);

        let election = LeaderElection::<HebEvent>::with_config(workspace, config)
            .elect()
            .map_err(HebError::Election)?;

        Ok(HebContext {
            data_dir: data_dir.to_path_buf(),
            election,
        })
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
        let seq = ctx.publish(&header, b"test body").unwrap();
        assert_eq!(seq, 1);

        // Verify it's in SQLite
        let events = ctx.replay(0, None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0.event_type, "pre_tool_use");
        assert_eq!(events[0].1, b"test body");
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

        let hooks = ctx.replay(0, Some("hook")).unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].0.category, EventCategory::Hook);
    }
}
