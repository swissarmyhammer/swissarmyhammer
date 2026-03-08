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
pub mod invalidation;
pub mod lsp_indexer;
pub mod ops;
pub mod ts_callgraph;
pub mod watcher;
pub mod workspace;

pub use cleanup::{startup_cleanup, CleanupStats};
pub use error::CodeContextError;
pub use invalidation::{reextract_file, refresh_edges, InvalidationAction};
pub use lsp_indexer::{
    build_qualified_path, build_symbol_id, flatten_symbols, mark_lsp_indexed, write_edges,
    write_symbols, CallEdge, FlatSymbol,
};
pub use ts_callgraph::{
    ensure_ts_symbols, extract_call_names, generate_ts_call_edges, resolve_callees,
    write_ts_edges, CallSite, ResolvedCallee,
};
pub use watcher::{FanoutWatcher, FileEvent, WatcherHandler};
pub use ops::find_symbol::{find_symbol, symbol_kind_name, SymbolLocation};
pub use ops::get_blastradius::{
    get_blastradius, AffectedSymbol, BlastRadius, BlastRadiusOptions, HopLevel,
};
pub use ops::get_callgraph::{
    get_callgraph, CallGraph, CallGraphDirection, CallGraphEdge, CallGraphNode, CallGraphOptions,
};
pub use ops::get_symbol::{
    get_symbol, GetSymbolOptions, GetSymbolResult, MatchTier, SymbolMatch,
};
pub use ops::grep_code::{grep_code, GrepMatch, GrepOptions, GrepResult, MatchPosition};
pub use ops::list_symbol::list_symbols;
pub use ops::search_symbol::{search_symbol, SearchSymbolMatch, SearchSymbolOptions};
pub use workspace::{CodeContextWorkspace, WorkspaceMode};
