//! File system watcher for keeping code-context DB in sync with filesystem.
//!
//! Watches for file additions, deletions, and modifications to keep the
//! code-context index synchronized with the actual filesystem state.
//!
//! NOTE: File watching is not yet integrated into the code-context tool.
//! For now, we rely on startup_cleanup() to reconcile the DB with the filesystem.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum FileEvent {
    Added(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
}
