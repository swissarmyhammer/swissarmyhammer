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

pub mod blocking;
pub mod cleanup;
pub mod config;
pub mod db;
pub mod error;
pub mod hints;
pub mod indexing;
pub mod invalidation;
pub mod lsp_communication;
pub mod lsp_indexer;
pub mod lsp_server;
pub mod lsp_worker;
pub mod ops;
pub mod ts_callgraph;
pub mod watcher;
pub mod workspace;

pub use cleanup::{startup_cleanup, CleanupStats};
pub use config::{
    load_code_context_config, load_code_context_config_from_paths,
    CodeContextConfigYaml, CodeContextSettings,
    CompiledCodeContextConfig, should_filter_stderr,
    StderrFilterRule, BUILTIN_CONFIG_YAML as CODE_CONTEXT_BUILTIN_CONFIG_YAML,
};
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
pub use ops::get_blastradius::{
    get_blastradius, AffectedSymbol, BlastRadius, BlastRadiusOptions, HopLevel,
};
pub use ops::get_callgraph::{
    get_callgraph, CallGraph, CallGraphDirection, CallGraphEdge, CallGraphNode, CallGraphOptions,
};
pub use ops::get_symbol::{
    get_symbol, symbol_kind_name, GetSymbolOptions, GetSymbolResult, MatchTier, SymbolLocation,
    SymbolMatch,
};
pub use ops::find_duplicates::{
    find_duplicates, ChunkRef, DuplicateGroup, DuplicateMatch, FindDuplicatesOptions,
    FindDuplicatesResult,
};
pub use ops::grep_code::{grep_code, GrepMatch, GrepOptions, GrepResult, MatchPosition};
pub use ops::query_ast::{
    query_ast, AstCapture, AstMatch, QueryAstOptions, QueryAstResult,
};
pub use ops::search_code::{
    search_code, serialize_embedding, SearchCodeMatch, SearchCodeOptions, SearchCodeResult,
};
pub use ops::list_symbol::list_symbols;
pub use ops::search_symbol::{search_symbol, SearchSymbolMatch, SearchSymbolOptions};
pub use blocking::{check_blocking_status, BlockingStatus, IndexLayer};
pub use hints::hint_for_operation;
pub use ops::status::{
    build_status, clear_status, get_status, BuildLayer, BuildStatusResult, ClearStatusResult,
    StatusReport,
};
pub use workspace::{CodeContextWorkspace, DbRef, SharedDb, WorkspaceMode};
pub use lsp_server::{
    detect_rust_analyzer, find_executable, start_lsp_server, LspServerConfig, LspServerHandle,
};
pub use lsp_communication::{LspJsonRpcClient, LspCollectionResult, collect_and_persist_symbols, parse_document_symbols};
pub use lsp_worker::{spawn_lsp_indexing_worker, LspWorkerConfig, SharedLspClient};
