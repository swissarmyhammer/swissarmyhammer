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
pub mod layered_context;
pub mod lsp_communication;
pub mod lsp_indexer;
pub mod lsp_server;
pub mod lsp_worker;
pub mod ops;
pub mod ts_callgraph;
pub mod watcher;
pub mod workspace;

#[cfg(test)]
pub(crate) mod test_fixtures;

pub use blocking::{check_blocking_status, BlockingStatus, IndexLayer};
pub use cleanup::{startup_cleanup, CleanupStats};
pub use config::{
    load_code_context_config, load_code_context_config_from_paths, should_filter_stderr,
    CodeContextConfigYaml, CodeContextSettings, CompiledCodeContextConfig, StderrFilterRule,
    BUILTIN_CONFIG_YAML as CODE_CONTEXT_BUILTIN_CONFIG_YAML,
};
pub use error::CodeContextError;
pub use hints::hint_for_operation;
pub use invalidation::InvalidationAction;
pub use layered_context::{
    CallEdgeInfo, ChunkInfo, DefinitionLocation, EnrichmentResult, FileEdit, LayeredContext,
    LspRange, SourceLayer, SymbolInfo, TextEdit,
};
pub use lsp_communication::{
    collect_and_persist_symbols, parse_document_symbols, LspCollectionResult, LspJsonRpcClient,
};
pub use lsp_indexer::{
    build_qualified_path, build_symbol_id, flatten_symbols, mark_lsp_indexed, write_edges,
    write_symbols, CallEdge, FlatSymbol,
};
pub use lsp_server::{
    builtin_lsp_yaml_sources, detect_rust_analyzer, find_executable, load_lsp_servers,
    start_lsp_server, LspServerConfig, LspServerHandle, OwnedLspServerSpec, LSP_REGISTRY,
};
pub use lsp_worker::{
    new_shutdown_flag, spawn_lsp_indexing_worker, LspWorkerConfig, SharedLspClient, ShutdownFlag,
};
pub use ops::find_duplicates::{
    find_duplicates, ChunkRef, DuplicateGroup, DuplicateMatch, FindDuplicatesOptions,
    FindDuplicatesResult,
};
pub use ops::get_blastradius::{
    get_blastradius, AffectedSymbol, BlastRadius, BlastRadiusOptions, HopLevel,
};
pub use ops::get_callgraph::{
    get_callgraph, CallGraph, CallGraphDirection, CallGraphEdge, CallGraphNode, CallGraphOptions,
};
pub use ops::get_code_actions::{
    get_code_actions, parse_code_actions, parse_workspace_edit, CodeAction, CodeActionsResult,
    GetCodeActionsOptions,
};
pub use ops::get_definition::{
    get_definition, parse_definition_locations, GetDefinitionOptions, GetDefinitionResult,
};
pub use ops::get_diagnostics::{
    get_diagnostics, parse_diagnostics_from_result, parse_publish_diagnostics,
    passes_severity_filter, Diagnostic, DiagnosticSeverity, DiagnosticsResult,
    GetDiagnosticsOptions,
};
pub use ops::get_hover::{
    get_hover, parse_hover_contents, parse_hover_range, GetHoverOptions, HoverResult,
};
pub use ops::get_implementations::{
    get_implementations, GetImplementationsOptions, GetImplementationsResult,
};
pub use ops::get_inbound_calls::{
    get_inbound_calls, GetInboundCallsOptions, InboundCallEntry, InboundCallsResult,
};
pub use ops::get_references::{
    get_references, FileReferenceGroup, GetReferencesOptions, ReferenceLocation, ReferencesResult,
};
pub use ops::get_rename_edits::{get_rename_edits, GetRenameEditsOptions, RenameEditsResult};
pub use ops::get_symbol::{
    get_symbol, symbol_kind_name, GetSymbolOptions, GetSymbolResult, MatchTier, SymbolLocation,
    SymbolMatch,
};
pub use ops::get_type_definition::{
    get_type_definition, GetTypeDefinitionOptions, GetTypeDefinitionResult,
};
pub use ops::grep_code::{grep_code, GrepMatch, GrepOptions, GrepResult, MatchPosition};
pub use ops::list_symbol::list_symbols;
pub use ops::lsp_helpers::parse_lsp_range;
pub use ops::query_ast::{query_ast, AstCapture, AstMatch, QueryAstOptions, QueryAstResult};
pub use ops::search_code::{
    search_code, serialize_embedding, SearchCodeMatch, SearchCodeOptions, SearchCodeResult,
};
pub use ops::search_symbol::{search_symbol, SearchSymbolMatch, SearchSymbolOptions};
pub use ops::status::{
    build_status, clear_status, distinct_extensions, get_status, BuildLayer, BuildStatusResult,
    ClearStatusResult, StatusReport,
};
pub use ops::workspace_symbol_live::{
    parse_workspace_symbols, workspace_symbol_live, WorkspaceSymbolLiveOptions,
    WorkspaceSymbolLiveResult, WorkspaceSymbolResult,
};
pub use ts_callgraph::{
    ensure_ts_symbols, extract_call_names, generate_ts_call_edges, resolve_callees, write_ts_edges,
    CallSite, ResolvedCallee,
};
pub use watcher::{FanoutWatcher, FileEvent, WatcherHandler};
pub use workspace::{CodeContextWorkspace, DbRef, SharedDb, WorkspaceMode};
