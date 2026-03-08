//! Unified code context index -- tree-sitter + LSP in a single SQLite database.
//!
//! Provides the `.code-context/` workspace layout, schema management,
//! and leader/reader coordination for the code context MCP tool.
//!
//! # Usage
//!
//! ```no_run
//! use std::path::Path;
//! use swissarmyhammer_code_context::CodeContextWorkspace;
//!
//! let ws = CodeContextWorkspace::open(Path::new("/my/project")).unwrap();
//! if ws.is_leader() {
//!     // This process owns the index -- run indexers.
//! }
//! let _conn = ws.db(); // read or write depending on mode
//! ```

pub mod cleanup;
pub mod db;
pub mod error;
pub mod lsp_indexer;
pub mod watcher;
pub mod workspace;

pub use cleanup::{startup_cleanup, CleanupStats};
pub use error::CodeContextError;
pub use lsp_indexer::{
    build_qualified_path, build_symbol_id, flatten_symbols, mark_lsp_indexed, write_edges,
    write_symbols, CallEdge, FlatSymbol,
};
pub use watcher::{FanoutWatcher, FileEvent, WatcherHandler};
pub use workspace::{CodeContextWorkspace, WorkspaceMode};
