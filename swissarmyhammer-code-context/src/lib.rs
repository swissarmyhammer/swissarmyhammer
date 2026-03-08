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

pub mod db;
pub mod error;
pub mod workspace;

pub use error::CodeContextError;
pub use workspace::{CodeContextWorkspace, WorkspaceMode};
