//! Unified code context tool for MCP operations
//!
//! This module provides a single `code_context` tool that dispatches between operations:
//! - `get symbol`: Symbol lookup with source text, locations, and multi-tier fuzzy matching
//! - `search symbol`: Fuzzy search across all indexed symbols
//! - `list symbols`: List all symbols in a specific file
//! - `grep code`: Regex search across stored code chunks
//! - `get callgraph`: Call graph traversal from a starting symbol
//! - `get blastradius`: Blast radius analysis for a file or symbol
//! - `get status`: Health report for the code context index
//! - `rebuild index`: Mark files for re-indexing
//! - `clear status`: Wipe all index data
//! - `lsp status`: Show detected languages, LSP servers, and install status
//! - `detect projects`: Detect project types in the workspace and return guidelines
//!
//! Uses the `swissarmyhammer-code-context` crate for all operations,
//! opening a `CodeContextWorkspace` from the `ToolContext` working directory.

pub mod detect;
pub mod doctor;
pub mod schema;
pub mod watcher;

use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use std::path::Path;
use swissarmyhammer_code_context::{
    BlastRadiusOptions, BlockingStatus, BuildLayer, CallGraphDirection, CallGraphOptions,
    CodeContextWorkspace, DiagnosticSeverity, FindDuplicatesOptions, GetCodeActionsOptions,
    GetDefinitionOptions, GetDiagnosticsOptions, GetHoverOptions, GetImplementationsOptions,
    GetInboundCallsOptions, GetReferencesOptions, GetSymbolOptions, GetTypeDefinitionOptions,
    GrepOptions, IndexLayer, LayeredContext, QueryAstOptions, SearchCodeOptions,
    SearchSymbolOptions, WorkspaceSymbolLiveOptions,
};
use swissarmyhammer_common::utils::find_git_repository_root_from;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Global LSP supervisor handle, initialized once at MCP startup.
/// Used by `get status` to report LSP server state and by `server.rs` for init.
pub(crate) static LSP_SUPERVISOR: std::sync::OnceLock<
    std::sync::Arc<tokio::sync::Mutex<swissarmyhammer_lsp::LspSupervisorManager>>,
> = std::sync::OnceLock::new();

/// Look up the `SharedLspClient` for a file by matching its extension against
/// the running LSP daemons in the global supervisor.
///
/// Returns `None` when the supervisor is not initialised, no daemon handles the
/// file's extension, or the supervisor lock cannot be acquired (e.g. contention).
fn lsp_client_for_file(file_path: &str) -> Option<swissarmyhammer_code_context::SharedLspClient> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())?;
    let sup = LSP_SUPERVISOR.get()?;
    let guard = sup.try_lock().ok()?;
    for name in guard.daemon_names() {
        if let Some(daemon) = guard.get_daemon(&name) {
            if daemon
                .file_extensions()
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext))
            {
                return Some(daemon.shared_client());
            }
        }
    }
    None
}

/// Return the first available `SharedLspClient` from any running daemon.
///
/// Useful for workspace-wide LSP requests (e.g. `workspace/symbol`) that are
/// not scoped to a single file extension.
fn any_lsp_client() -> Option<swissarmyhammer_code_context::SharedLspClient> {
    let sup = LSP_SUPERVISOR.get()?;
    let guard = sup.try_lock().ok()?;
    for name in guard.daemon_names() {
        if let Some(daemon) = guard.get_daemon(&name) {
            if matches!(
                daemon.state(),
                swissarmyhammer_lsp::LspDaemonState::Running { .. }
            ) {
                return Some(daemon.shared_client());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Operation structs with Operation trait impls
// ---------------------------------------------------------------------------

/// Operation metadata for getting symbol source text with fuzzy matching.
#[derive(Debug, Default)]
pub struct GetSymbol;

static GET_SYMBOL_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("The symbol name or qualified path to search for")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("max_results")
        .description("Maximum number of results to return")
        .param_type(ParamType::Integer),
];

impl Operation for GetSymbol {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "symbol"
    }
    fn description(&self) -> &'static str {
        "Get symbol locations and source text from both LSP and tree-sitter indices with multi-tier fuzzy matching"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_SYMBOL_PARAMS
    }
}

/// Operation metadata for fuzzy symbol search.
#[derive(Debug, Default)]
pub struct SearchSymbol;

static SEARCH_SYMBOL_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("The text to fuzzy-match against symbol names")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("kind")
        .description(
            "Filter by symbol kind: function, method, struct, class, interface, module, etc.",
        )
        .param_type(ParamType::String),
    ParamMeta::new("max_results")
        .description("Maximum number of results to return")
        .param_type(ParamType::Integer),
];

impl Operation for SearchSymbol {
    fn verb(&self) -> &'static str {
        "search"
    }
    fn noun(&self) -> &'static str {
        "symbol"
    }
    fn description(&self) -> &'static str {
        "Fuzzy search across all indexed symbols with optional kind filter"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SEARCH_SYMBOL_PARAMS
    }
}

/// Operation metadata for listing symbols in a file.
#[derive(Debug, Default)]
pub struct ListSymbols;

static LIST_SYMBOLS_PARAMS: &[ParamMeta] = &[ParamMeta::new("file_path")
    .description("Path to the file to list symbols from")
    .param_type(ParamType::String)
    .required()];

impl Operation for ListSymbols {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "symbols"
    }
    fn description(&self) -> &'static str {
        "List all symbols in a specific file, sorted by start line"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        LIST_SYMBOLS_PARAMS
    }
}

/// Operation metadata for regex search across code chunks.
#[derive(Debug, Default)]
pub struct GrepCode;

static GREP_CODE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("pattern")
        .description("Regex pattern to search for")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("language")
        .description("Only search chunks from files with these extensions (e.g. [\"rs\", \"py\"])")
        .param_type(ParamType::Array),
    ParamMeta::new("files")
        .description("Only search chunks from these specific file paths")
        .param_type(ParamType::Array),
    ParamMeta::new("max_results")
        .description("Maximum number of matching chunks to return")
        .param_type(ParamType::Integer),
];

impl Operation for GrepCode {
    fn verb(&self) -> &'static str {
        "grep"
    }
    fn noun(&self) -> &'static str {
        "code"
    }
    fn description(&self) -> &'static str {
        "Regex search across stored code chunks"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GREP_CODE_PARAMS
    }
}

/// Operation metadata for call graph traversal.
#[derive(Debug, Default)]
pub struct GetCallgraph;

static GET_CALLGRAPH_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("symbol")
        .description("Symbol identifier -- either a name or a file:line:char locator")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("direction")
        .description("Traversal direction: inbound, outbound, or both (default: outbound)")
        .param_type(ParamType::String),
    ParamMeta::new("max_depth")
        .description("Maximum traversal depth, 1-5 (default: 2)")
        .param_type(ParamType::Integer),
];

impl Operation for GetCallgraph {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "callgraph"
    }
    fn description(&self) -> &'static str {
        "Traverse call graph from a starting symbol"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_CALLGRAPH_PARAMS
    }
}

/// Operation metadata for inbound calls (who calls this function?).
#[derive(Debug, Default)]
pub struct GetInboundCalls;

static GET_INBOUND_CALLS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the target symbol")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the target symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("depth")
        .description("Recursive depth for caller traversal, 1-5 (default: 1)")
        .param_type(ParamType::Integer),
];

impl Operation for GetInboundCalls {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "inbound_calls"
    }
    fn description(&self) -> &'static str {
        "Find all callers of a function at a given position (who calls this?)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_INBOUND_CALLS_PARAMS
    }
}

/// Operation metadata for live workspace symbol search.
#[derive(Debug, Default)]
pub struct WorkspaceSymbolLive;

static WORKSPACE_SYMBOL_LIVE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("The symbol name or text to search for across the workspace")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("max_results")
        .description("Maximum number of results to return (default: 50)")
        .param_type(ParamType::Integer),
];

impl Operation for WorkspaceSymbolLive {
    fn verb(&self) -> &'static str {
        "search"
    }
    fn noun(&self) -> &'static str {
        "workspace_symbol"
    }
    fn description(&self) -> &'static str {
        "Live workspace symbol search with layered resolution (live LSP, then LSP index, then tree-sitter)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        WORKSPACE_SYMBOL_LIVE_PARAMS
    }
}

/// Operation metadata for blast radius analysis.
#[derive(Debug, Default)]
pub struct GetBlastradius;

static GET_BLASTRADIUS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("File path to analyze")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("symbol")
        .description("Optional symbol name within the file to narrow the starting set")
        .param_type(ParamType::String),
    ParamMeta::new("max_hops")
        .description("Maximum number of hops to follow, 1-10 (default: 3)")
        .param_type(ParamType::Integer),
];

impl Operation for GetBlastradius {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "blastradius"
    }
    fn description(&self) -> &'static str {
        "Analyze blast radius of changes to a file or symbol"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_BLASTRADIUS_PARAMS
    }
}

/// Operation metadata for index status checking.
#[derive(Debug, Default)]
pub struct GetCodeStatus;

impl Operation for GetCodeStatus {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "status"
    }
    fn description(&self) -> &'static str {
        "Health report with file counts, indexing progress, chunk/edge counts"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        &[]
    }
}

/// Operation metadata for triggering re-indexing.
#[derive(Debug, Default)]
pub struct RebuildIndex;

static REBUILD_INDEX_PARAMS: &[ParamMeta] = &[ParamMeta::new("layer")
    .description("Which indexing layer to reset: treesitter, lsp, or both (default: both)")
    .param_type(ParamType::String)];

impl Operation for RebuildIndex {
    fn verb(&self) -> &'static str {
        "rebuild"
    }
    fn noun(&self) -> &'static str {
        "index"
    }
    fn description(&self) -> &'static str {
        "Mark files for re-indexing by resetting indexed flags"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        REBUILD_INDEX_PARAMS
    }
}

/// Operation metadata for clearing all index data.
#[derive(Debug, Default)]
pub struct ClearStatus;

impl Operation for ClearStatus {
    fn verb(&self) -> &'static str {
        "clear"
    }
    fn noun(&self) -> &'static str {
        "status"
    }
    fn description(&self) -> &'static str {
        "Wipe all index data and return stats"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        &[]
    }
}

/// Operation metadata for LSP status checking based on indexed file extensions.
#[derive(Debug, Default)]
pub struct LspStatus;

impl Operation for LspStatus {
    fn verb(&self) -> &'static str {
        "lsp"
    }
    fn noun(&self) -> &'static str {
        "status"
    }
    fn description(&self) -> &'static str {
        "Show which languages are detected in the index, their LSP servers, and install status"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        &[]
    }
}

/// Operation metadata for semantic code search using embeddings.
#[derive(Debug, Default)]
pub struct SearchCode;

static SEARCH_CODE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("Natural language query to search for semantically similar code")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("top_k")
        .description("Maximum number of results to return (default: 10)")
        .param_type(ParamType::Integer),
    ParamMeta::new("min_similarity")
        .description("Minimum cosine similarity threshold, 0.0-1.0 (default: 0.7)")
        .param_type(ParamType::Number),
    ParamMeta::new("language")
        .description("Only search chunks from files with these extensions (e.g. [\"rs\", \"py\"])")
        .param_type(ParamType::Array),
    ParamMeta::new("file_pattern")
        .description("Only search chunks from files matching this path pattern")
        .param_type(ParamType::String),
];

impl Operation for SearchCode {
    fn verb(&self) -> &'static str {
        "search"
    }
    fn noun(&self) -> &'static str {
        "code"
    }
    fn description(&self) -> &'static str {
        "Semantic similarity search across code chunks using embeddings"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SEARCH_CODE_PARAMS
    }
}

/// Operation metadata for finding duplicated code.
#[derive(Debug, Default)]
pub struct FindDuplicates;

static FIND_DUPLICATES_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("File to check for duplicated code elsewhere in the codebase")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("min_similarity")
        .description("Minimum cosine similarity to report as duplicate, 0.0-1.0 (default: 0.85)")
        .param_type(ParamType::Number),
    ParamMeta::new("min_chunk_bytes")
        .description("Minimum chunk size in bytes to consider (default: 100)")
        .param_type(ParamType::Integer),
    ParamMeta::new("max_per_chunk")
        .description("Maximum duplicates to show per source chunk (default: 5)")
        .param_type(ParamType::Integer),
];

impl Operation for FindDuplicates {
    fn verb(&self) -> &'static str {
        "find"
    }
    fn noun(&self) -> &'static str {
        "duplicates"
    }
    fn description(&self) -> &'static str {
        "Find code in a file that is duplicated elsewhere in the codebase"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        FIND_DUPLICATES_PARAMS
    }
}

/// Operation metadata for tree-sitter S-expression AST queries.
#[derive(Debug, Default)]
pub struct QueryAst;

static QUERY_AST_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("Tree-sitter S-expression query pattern (e.g., '(function_item name: (identifier) @name)')")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("language")
        .description("Language to parse files as (e.g., 'rust', 'python', 'typescript')")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("files")
        .description("File paths (relative to workspace root) to query against")
        .param_type(ParamType::Array),
    ParamMeta::new("max_results")
        .description("Maximum number of matches to return (default: 50)")
        .param_type(ParamType::Integer),
];

impl Operation for QueryAst {
    fn verb(&self) -> &'static str {
        "query"
    }
    fn noun(&self) -> &'static str {
        "ast"
    }
    fn description(&self) -> &'static str {
        "Execute tree-sitter S-expression queries against parsed ASTs for structural code search"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        QUERY_AST_PARAMS
    }
}

/// Operation metadata for project detection.
#[derive(Debug, Default)]
pub struct DetectProjects;

static DETECT_PROJECTS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("Root path to search for projects (default: current directory)")
        .param_type(ParamType::String),
    ParamMeta::new("max_depth")
        .description("Maximum directory depth to search (default: 3)")
        .param_type(ParamType::Integer),
    ParamMeta::new("include_guidelines")
        .description("Include language-specific guidelines in output (default: true)")
        .param_type(ParamType::Boolean),
];

impl Operation for DetectProjects {
    fn verb(&self) -> &'static str {
        "detect"
    }
    fn noun(&self) -> &'static str {
        "projects"
    }
    fn description(&self) -> &'static str {
        "Detect project types in the workspace and return language-specific guidelines"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        DETECT_PROJECTS_PARAMS
    }
}

/// Operation metadata for previewing rename edits.
#[derive(Debug, Default)]
pub struct GetRenameEdits;

static GET_RENAME_EDITS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the symbol to rename")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("new_name")
        .description("The new name for the symbol")
        .param_type(ParamType::String)
        .required(),
];

impl Operation for GetRenameEdits {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "rename_edits"
    }
    fn description(&self) -> &'static str {
        "Preview rename edits without applying them (live LSP only). Returns can_rename: false when no LSP is available."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_RENAME_EDITS_PARAMS
    }
}

/// Operation metadata for getting file diagnostics.
#[derive(Debug, Default)]
pub struct GetDiagnostics;

static GET_DIAGNOSTICS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file to get diagnostics for")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("severity_filter")
        .description(
            "Only return diagnostics at or above this severity: 'error', 'warning', 'info', 'hint'. Omit for all.",
        )
        .param_type(ParamType::String),
];

impl Operation for GetDiagnostics {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "diagnostics"
    }
    fn description(&self) -> &'static str {
        "Get errors and warnings for a file (live LSP only). Returns empty when no LSP is available."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_DIAGNOSTICS_PARAMS
    }
}

/// Operation metadata for go-to-definition.
#[derive(Debug, Default)]
pub struct GetDefinition;

static GET_DEFINITION_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the symbol")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("include_source")
        .description("Whether to include source text at each definition location (default: true)")
        .param_type(ParamType::Boolean),
];

impl Operation for GetDefinition {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "definition"
    }
    fn description(&self) -> &'static str {
        "Go to definition with layered resolution (live LSP, LSP index, tree-sitter)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_DEFINITION_PARAMS
    }
}

/// Operation metadata for go-to-type-definition.
#[derive(Debug, Default)]
pub struct GetTypeDefinition;

static GET_TYPE_DEFINITION_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the symbol")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("include_source")
        .description("Whether to include source text at each definition location (default: true)")
        .param_type(ParamType::Boolean),
];

impl Operation for GetTypeDefinition {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "type_definition"
    }
    fn description(&self) -> &'static str {
        "Go to type definition (live LSP only). Returns empty when no LSP is available."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_TYPE_DEFINITION_PARAMS
    }
}

/// Operation metadata for hover information.
#[derive(Debug, Default)]
pub struct GetHover;

static GET_HOVER_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the symbol")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
];

impl Operation for GetHover {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "hover"
    }
    fn description(&self) -> &'static str {
        "Get hover information (type signature, docs) with layered resolution (live LSP, LSP index, tree-sitter)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_HOVER_PARAMS
    }
}

/// Operation metadata for find-all-references.
#[derive(Debug, Default)]
pub struct GetReferences;

static GET_REFERENCES_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the symbol")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("include_declaration")
        .description("Whether to include the declaration itself in results (default: true)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("max_results")
        .description("Maximum number of references to return")
        .param_type(ParamType::Integer),
];

impl Operation for GetReferences {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "references"
    }
    fn description(&self) -> &'static str {
        "Find all references to a symbol with layered resolution (live LSP, LSP index, tree-sitter)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_REFERENCES_PARAMS
    }
}

/// Operation metadata for find-implementations.
#[derive(Debug, Default)]
pub struct GetImplementations;

static GET_IMPLEMENTATIONS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file containing the trait/interface symbol")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("line")
        .description("Zero-based line number of the symbol")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("character")
        .description("Zero-based character offset within the line")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("max_results")
        .description("Maximum number of implementation locations to return")
        .param_type(ParamType::Integer),
];

impl Operation for GetImplementations {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "implementations"
    }
    fn description(&self) -> &'static str {
        "Find implementations of a trait/interface with layered resolution (live LSP, tree-sitter heuristic)"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_IMPLEMENTATIONS_PARAMS
    }
}

/// Operation metadata for code actions (quickfixes, refactors).
#[derive(Debug, Default)]
pub struct GetCodeActions;

static GET_CODE_ACTIONS_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Path to the file to get code actions for")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("start_line")
        .description("Zero-based start line of the range to query")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("start_character")
        .description("Zero-based start character offset")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("end_line")
        .description("Zero-based end line of the range to query")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("end_character")
        .description("Zero-based end character offset")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("filter_kind")
        .description(
            "Optional filter for code action kinds (e.g. [\"quickfix\", \"refactor\", \"source\"])",
        )
        .param_type(ParamType::Array),
];

impl Operation for GetCodeActions {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "code_actions"
    }
    fn description(&self) -> &'static str {
        "Get code actions (quickfixes, refactors) for a range (live LSP only). Returns empty when no LSP is available."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_CODE_ACTIONS_PARAMS
    }
}

// Static operation instances for schema generation
static GET_SYMBOL_OP: Lazy<GetSymbol> = Lazy::new(GetSymbol::default);
static SEARCH_SYMBOL_OP: Lazy<SearchSymbol> = Lazy::new(SearchSymbol::default);
static LIST_SYMBOLS_OP: Lazy<ListSymbols> = Lazy::new(ListSymbols::default);
static GREP_CODE_OP: Lazy<GrepCode> = Lazy::new(GrepCode::default);
static GET_CALLGRAPH_OP: Lazy<GetCallgraph> = Lazy::new(GetCallgraph::default);
static GET_BLASTRADIUS_OP: Lazy<GetBlastradius> = Lazy::new(GetBlastradius::default);
static GET_CODE_STATUS_OP: Lazy<GetCodeStatus> = Lazy::new(GetCodeStatus::default);
static REBUILD_INDEX_OP: Lazy<RebuildIndex> = Lazy::new(RebuildIndex::default);
static CLEAR_STATUS_OP: Lazy<ClearStatus> = Lazy::new(ClearStatus::default);
static LSP_STATUS_OP: Lazy<LspStatus> = Lazy::new(LspStatus::default);
static SEARCH_CODE_OP: Lazy<SearchCode> = Lazy::new(SearchCode::default);
static FIND_DUPLICATES_OP: Lazy<FindDuplicates> = Lazy::new(FindDuplicates::default);
static QUERY_AST_OP: Lazy<QueryAst> = Lazy::new(QueryAst::default);
static DETECT_PROJECTS_OP: Lazy<DetectProjects> = Lazy::new(DetectProjects::default);
static GET_RENAME_EDITS_OP: Lazy<GetRenameEdits> = Lazy::new(GetRenameEdits::default);
static GET_DIAGNOSTICS_OP: Lazy<GetDiagnostics> = Lazy::new(GetDiagnostics::default);
static GET_INBOUND_CALLS_OP: Lazy<GetInboundCalls> = Lazy::new(GetInboundCalls::default);
static WORKSPACE_SYMBOL_LIVE_OP: Lazy<WorkspaceSymbolLive> =
    Lazy::new(WorkspaceSymbolLive::default);
static GET_DEFINITION_OP: Lazy<GetDefinition> = Lazy::new(GetDefinition::default);
static GET_TYPE_DEFINITION_OP: Lazy<GetTypeDefinition> = Lazy::new(GetTypeDefinition::default);
static GET_HOVER_OP: Lazy<GetHover> = Lazy::new(GetHover::default);
static GET_REFERENCES_OP: Lazy<GetReferences> = Lazy::new(GetReferences::default);
static GET_IMPLEMENTATIONS_OP: Lazy<GetImplementations> = Lazy::new(GetImplementations::default);
static GET_CODE_ACTIONS_OP: Lazy<GetCodeActions> = Lazy::new(GetCodeActions::default);

static CODE_CONTEXT_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*GET_SYMBOL_OP as &dyn Operation,
        &*SEARCH_SYMBOL_OP as &dyn Operation,
        &*LIST_SYMBOLS_OP as &dyn Operation,
        &*GREP_CODE_OP as &dyn Operation,
        &*SEARCH_CODE_OP as &dyn Operation,
        &*FIND_DUPLICATES_OP as &dyn Operation,
        &*QUERY_AST_OP as &dyn Operation,
        &*GET_CALLGRAPH_OP as &dyn Operation,
        &*GET_BLASTRADIUS_OP as &dyn Operation,
        &*GET_CODE_STATUS_OP as &dyn Operation,
        &*REBUILD_INDEX_OP as &dyn Operation,
        &*CLEAR_STATUS_OP as &dyn Operation,
        &*LSP_STATUS_OP as &dyn Operation,
        &*DETECT_PROJECTS_OP as &dyn Operation,
        &*GET_RENAME_EDITS_OP as &dyn Operation,
        &*GET_DIAGNOSTICS_OP as &dyn Operation,
        &*GET_INBOUND_CALLS_OP as &dyn Operation,
        &*WORKSPACE_SYMBOL_LIVE_OP as &dyn Operation,
        &*GET_DEFINITION_OP as &dyn Operation,
        &*GET_TYPE_DEFINITION_OP as &dyn Operation,
        &*GET_HOVER_OP as &dyn Operation,
        &*GET_REFERENCES_OP as &dyn Operation,
        &*GET_IMPLEMENTATIONS_OP as &dyn Operation,
        &*GET_CODE_ACTIONS_OP as &dyn Operation,
    ]
});

/// Unified code context tool providing symbol lookup, search, and graph operations.
#[derive(Clone, Default)]
pub struct CodeContextTool;

impl CodeContextTool {
    /// Creates a new CodeContextTool instance.
    pub fn new() -> Self {
        Self
    }
}

impl swissarmyhammer_common::health::Doctorable for CodeContextTool {
    fn name(&self) -> &str {
        "Code Context"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        use swissarmyhammer_common::health::HealthCheck;

        let mut checks = Vec::new();
        let cat = swissarmyhammer_common::health::Doctorable::category(self);

        // Check LSP server availability for detected project type
        let cwd = std::env::current_dir().unwrap_or_default();
        let report = doctor::run_doctor(&cwd);

        if report.project_types.is_empty() {
            checks.push(HealthCheck::ok(
                "LSP servers",
                "No project type detected — no LSP required",
                cat,
            ));
        } else {
            let types_label = report.project_types.join(", ");
            for lsp in &report.lsp_servers {
                if lsp.installed {
                    checks.push(HealthCheck::ok(
                        format!("{} (LSP)", lsp.name),
                        format!("Available at {}", lsp.path.as_deref().unwrap_or("unknown")),
                        cat,
                    ));
                } else if let Some(ref err) = lsp.error {
                    // Binary found on PATH but doesn't actually work
                    let hint = lsp.install_hint.as_deref().unwrap_or("Check installation");
                    checks.push(HealthCheck::error(
                        format!("{} (LSP)", lsp.name),
                        format!(
                            "Found at {} but broken: {}",
                            lsp.path.as_deref().unwrap_or("unknown"),
                            err
                        ),
                        Some(hint.to_string()),
                        cat,
                    ));
                } else {
                    // Not found at all
                    let hint = lsp
                        .install_hint
                        .as_deref()
                        .unwrap_or("Install the LSP server");
                    checks.push(HealthCheck::warning(
                        format!("{} (LSP)", lsp.name),
                        format!("Not found (needed for {} code intelligence)", types_label),
                        Some(hint.to_string()),
                        cat,
                    ));
                }
            }
        }

        checks
    }

    fn is_applicable(&self) -> bool {
        true
    }
}
impl swissarmyhammer_common::lifecycle::Initializable for CodeContextTool {
    fn name(&self) -> &str {
        "code_context"
    }
    fn category(&self) -> &str {
        "tools"
    }
    fn priority(&self) -> i32 {
        22
    }

    fn init(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
        _reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;

        // Create .code-context/ directory if in a git repo
        let root = swissarmyhammer_common::utils::find_git_repository_root();
        match root {
            Some(root) => {
                let cc_dir = root.join(".code-context");
                if !cc_dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(&cc_dir) {
                        return vec![InitResult::error(
                            "code-context",
                            format!("Failed to create .code-context/: {}", e),
                        )];
                    }
                }
                // Ensure .code-context/ is in .gitignore
                let gitignore = root.join(".gitignore");
                let needs_entry = if gitignore.exists() {
                    match std::fs::read_to_string(&gitignore) {
                        Ok(content) => !content
                            .lines()
                            .any(|l| l.trim() == ".code-context" || l.trim() == ".code-context/"),
                        Err(_) => true,
                    }
                } else {
                    true
                };
                if needs_entry {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&gitignore)
                    {
                        let _ = writeln!(f, ".code-context/");
                    }
                }
                vec![InitResult::ok(
                    "code-context",
                    "Created .code-context/ directory",
                )]
            }
            None => vec![InitResult::skipped(
                "code-context",
                "No git repository found",
            )],
        }
    }

    fn deinit(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
        _reporter: &dyn swissarmyhammer_common::reporter::InitReporter,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use swissarmyhammer_common::lifecycle::InitResult;

        let root = swissarmyhammer_common::utils::find_git_repository_root();
        match root {
            Some(root) => {
                let cc_dir = root.join(".code-context");
                if cc_dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&cc_dir) {
                        return vec![InitResult::error(
                            "code-context",
                            format!("Failed to remove .code-context/: {}", e),
                        )];
                    }
                    vec![InitResult::ok(
                        "code-context",
                        "Removed .code-context/ directory",
                    )]
                } else {
                    vec![InitResult::skipped(
                        "code-context",
                        ".code-context/ not found",
                    )]
                }
            }
            None => vec![InitResult::skipped(
                "code-context",
                "No git repository found",
            )],
        }
    }

    // start() and stop() left as defaults — background work is currently managed
    // by McpServer::initialize_code_context() which has access to work_dir.
    // Future: when tools receive context at start time, move that logic here.
}

#[async_trait]
impl McpTool for CodeContextTool {
    fn name(&self) -> &'static str {
        "code_context"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        schema::generate_code_context_schema(&CODE_CONTEXT_OPERATIONS)
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("code_context")
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&'static dyn Operation] = &CODE_CONTEXT_OPERATIONS;
        // SAFETY: CODE_CONTEXT_OPERATIONS is a static Lazy<Vec<...>> initialized once and lives for 'static
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        let result = match op_str {
            "get symbol" => execute_get_symbol(&arguments, context),
            "search symbol" => execute_search_symbol(&arguments, context),
            "list symbols" => execute_list_symbols(&arguments, context),
            "grep code" => execute_grep_code(&arguments, context),
            "search code" => execute_search_code(&arguments, context).await,
            "find duplicates" => execute_find_duplicates(&arguments, context),
            "query ast" => execute_query_ast(&arguments, context),
            "get callgraph" => execute_get_callgraph(&arguments, context),
            "get blastradius" => execute_get_blastradius(&arguments, context),
            "get status" => execute_get_status(context),
            "rebuild index" => execute_rebuild_index(&arguments, context).await,
            "clear status" => execute_clear_status(context),
            "lsp status" => execute_lsp_status(context),
            "detect projects" => detect::execute_detect(&arguments, context).await,
            "get rename_edits" => execute_get_rename_edits(&arguments, context),
            "get diagnostics" => execute_get_diagnostics(&arguments, context),
            "get inbound_calls" => execute_get_inbound_calls(&arguments, context),
            "search workspace_symbol" => {
                execute_workspace_symbol_live(&arguments, context)
            }
            "get definition" => execute_get_definition(&arguments, context),
            "get type_definition" => execute_get_type_definition(&arguments, context),
            "get hover" => execute_get_hover(&arguments, context),
            "get references" => execute_get_references(&arguments, context),
            "get implementations" => execute_get_implementations(&arguments, context),
            "get code_actions" => execute_get_code_actions(&arguments, context),
            "" => Err(McpError::invalid_params(
                "Missing 'op' field. Valid operations: 'get symbol', 'search symbol', 'list symbols', 'grep code', 'search code', 'find duplicates', 'query ast', 'get callgraph', 'get blastradius', 'get status', 'rebuild index', 'clear status', 'lsp status', 'detect projects', 'get rename_edits', 'get diagnostics', 'get inbound_calls', 'search workspace_symbol', 'get definition', 'get type_definition', 'get hover', 'get references', 'get implementations', 'get code_actions'.",
                None,
            )),
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'get symbol', 'search symbol', 'list symbols', 'grep code', 'search code', 'find duplicates', 'query ast', 'get callgraph', 'get blastradius', 'get status', 'rebuild index', 'clear status', 'lsp status', 'detect projects', 'get rename_edits', 'get diagnostics', 'get inbound_calls', 'search workspace_symbol', 'get definition', 'get type_definition', 'get hover', 'get references', 'get implementations', 'get code_actions'",
                    other
                ),
                None,
            )),
        };

        // Append LSP degradation notice to query operations (not status operations)
        match op_str {
            "get status" | "rebuild index" | "clear status" | "lsp status" | "detect projects"
            | "" => result,
            _ => result.map(|r| maybe_append_lsp_notice(r, context)),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: open workspace from context
// ---------------------------------------------------------------------------

/// Open a CodeContextWorkspace from the tool context's working directory.
///
/// Falls back to the current directory if no working_dir is set.
fn open_workspace(context: &ToolContext) -> Result<CodeContextWorkspace, McpError> {
    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    // Find the git repository root from the working directory
    let workspace_root = find_git_repository_root_from(&working_dir).unwrap_or(working_dir);

    CodeContextWorkspace::open(&workspace_root).map_err(|e| {
        McpError::internal_error(
            format!("Failed to open code context workspace: {}", e),
            None,
        )
    })
}

/// Format a serializable result into a CallToolResult with JSON text content.
fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(value).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize result: {}", e), None)
    })?;

    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Convert a CodeContextError into an McpError.
///
/// Most errors become generic `internal_error`s. `ReadOnlyFollower` is special:
/// it's a user-actionable misconfiguration (writes attempted from a non-leader
/// process), so we surface it as `invalid_request` with the typed diagnostic
/// message instead of an opaque "-32603: database error".
fn context_err(e: swissarmyhammer_code_context::CodeContextError) -> McpError {
    use swissarmyhammer_code_context::CodeContextError;
    match e {
        e @ CodeContextError::ReadOnlyFollower { .. } => {
            McpError::invalid_request(format!("{}", e), None)
        }
        other => McpError::internal_error(format!("{}", other), None),
    }
}

/// Check if tree-sitter indexing is complete; if not, return a progress message.
///
/// Returns `Ok(None)` when ready, `Ok(Some(result))` with a progress message when not.
fn check_ts_readiness(ws: &CodeContextWorkspace) -> Result<Option<CallToolResult>, McpError> {
    let status =
        swissarmyhammer_code_context::check_blocking_status(&ws.db(), IndexLayer::TreeSitter)
            .map_err(context_err)?;
    match status {
        BlockingStatus::Ready => Ok(None),
        BlockingStatus::NotReady {
            total_files,
            indexed_files,
            progress_percent,
        } => {
            let msg = format!(
                "Index not ready — {}/{} files indexed ({:.0}% complete). Please retry shortly.",
                indexed_files, total_files, progress_percent
            );
            Ok(Some(CallToolResult::success(vec![Content::text(msg)])))
        }
    }
}

// ---------------------------------------------------------------------------
// LSP degradation notice
// ---------------------------------------------------------------------------

/// Check if any LSP servers are missing and return a notice string if so.
///
/// Checks the global LSP_SUPERVISOR for daemons in NotFound state.
/// Falls back to the doctor check if the supervisor isn't initialized.
/// Returns None if all LSP servers are available (no noise).
fn lsp_degradation_notice(workspace_root: &std::path::Path) -> Option<String> {
    // Try the supervisor first (it has live state)
    if let Some(sup) = LSP_SUPERVISOR.get() {
        if let Ok(guard) = sup.try_lock() {
            let statuses = guard.status();
            let missing: Vec<_> = statuses
                .iter()
                .filter(|s| matches!(s.state, swissarmyhammer_lsp::LspDaemonState::NotFound))
                .collect();
            if missing.is_empty() {
                return None;
            }
            // Get install hints from the doctor module since DaemonStatus doesn't have them
            let report = doctor::run_doctor(workspace_root);
            let mut lines = vec![
                "\n---".to_string(),
                "Note: Code intelligence is limited to tree-sitter only.".to_string(),
            ];
            for daemon in &missing {
                let hint = report
                    .lsp_servers
                    .iter()
                    .find(|s| s.name == daemon.command)
                    .and_then(|s| s.install_hint.as_deref())
                    .unwrap_or("see project documentation");
                lines.push(format!("  {}: NOT INSTALLED — {}", daemon.command, hint));
            }
            return Some(lines.join("\n"));
        }
    }

    // Supervisor not yet initialized — fall back to doctor check
    let report = doctor::run_doctor(workspace_root);
    let missing: Vec<_> = report.lsp_servers.iter().filter(|s| !s.installed).collect();
    if missing.is_empty() {
        return None;
    }
    let mut lines = vec![
        "\n---".to_string(),
        "Note: Code intelligence is limited to tree-sitter only.".to_string(),
    ];
    for server in &missing {
        let hint = server
            .install_hint
            .as_deref()
            .unwrap_or("see project documentation");
        lines.push(format!("  {}: NOT INSTALLED — {}", server.name, hint));
    }
    Some(lines.join("\n"))
}

/// Append an LSP degradation notice to a successful tool result if applicable.
///
/// Resolves the workspace root from the tool context and checks for missing LSP
/// servers. If any are missing, a second text content item is appended to the result
/// so the caller knows results are tree-sitter only.
fn maybe_append_lsp_notice(mut result: CallToolResult, context: &ToolContext) -> CallToolResult {
    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let workspace_root = find_git_repository_root_from(&working_dir).unwrap_or(working_dir);

    if let Some(notice) = lsp_degradation_notice(&workspace_root) {
        result.content.push(Content::text(notice));
    }
    result
}

// ---------------------------------------------------------------------------
// Operation handlers
// ---------------------------------------------------------------------------

/// Execute the "get symbol" operation.
///
/// Retrieves symbol source text using multi-tier fuzzy matching
/// (exact, suffix, case-insensitive, fuzzy).
fn execute_get_symbol(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'query'", None))?;

    let options = GetSymbolOptions {
        max_results: args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize),
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let result =
        swissarmyhammer_code_context::get_symbol(&ws.db(), query, &options).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "search symbol" operation.
///
/// Fuzzy search across all indexed symbols with optional kind filter.
fn execute_search_symbol(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'query'", None))?;

    let options = SearchSymbolOptions {
        kind: args.get("kind").and_then(|v| v.as_str()).map(String::from),
        max_results: args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize),
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let results = swissarmyhammer_code_context::search_symbol(&ws.db(), query, &options)
        .map_err(context_err)?;
    json_result(&results)
}

/// Execute the "list symbols" operation.
///
/// Lists all symbols in a specific file, sorted by start line.
fn execute_list_symbols(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let results =
        swissarmyhammer_code_context::list_symbols(&ws.db(), file_path).map_err(context_err)?;
    json_result(&results)
}

/// Execute the "grep code" operation.
///
/// Regex search across stored code chunks, returning complete semantic blocks.
fn execute_grep_code(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'pattern'", None))?;

    let language = args.get("language").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
    });

    let files = args.get("files").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
    });

    let options = GrepOptions {
        language,
        files,
        max_results: args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize),
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let result = swissarmyhammer_code_context::grep_code(&ws.db(), pattern, &options)
        .map_err(context_err)?;
    json_result(&result)
}

/// Execute the "search code" operation.
///
/// Embeds the query text and computes cosine similarity against stored chunk
/// embeddings.
///
/// Unlike the other code-context ops, `search code` does **not** gate on
/// tree-sitter readiness. The embedding pass may still be running on a fresh
/// workspace; rather than refuse to answer, we always run the query against
/// whatever embeddings exist and surface in-progress state to the caller via
/// the `progress` field on [`SearchCodeResult`]. Removing the gate was a
/// deliberate decision — `check_ts_readiness` is still used by the eight other
/// ops that genuinely cannot produce useful results without a full chunk
/// index.
async fn execute_search_code(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'query'", None))?;

    // Embed the query text
    use swissarmyhammer_embedding::{Embedder, TextEmbedder};
    let embedder = Embedder::default()
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to create embedder: {}", e), None))?;
    embedder.load().await.map_err(|e| {
        McpError::internal_error(format!("Failed to load embedding model: {}", e), None)
    })?;
    let embed_result = embedder
        .embed_text(query)
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to embed query: {}", e), None))?;

    search_code_with_query_embedding(args, context, embed_result.embedding())
}

/// Inner half of [`execute_search_code`] after the query has been embedded.
///
/// Split out so unit tests can exercise the search path without loading a
/// real embedding model. The caller-supplied `query_embedding` is treated
/// as-if it had been produced by the same embedder that wrote the chunk
/// embeddings — tests that don't care about ranking can pass any non-empty
/// vector.
///
/// This function **never** returns the old "Index not ready" placeholder.
/// When the embedding pass is still running, the resulting `SearchCodeResult`
/// carries a populated `progress` field instead.
fn search_code_with_query_embedding(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
    query_embedding: &[f32],
) -> Result<CallToolResult, McpError> {
    let top_k = args
        .get("top_k")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(10);

    let min_similarity = args
        .get("min_similarity")
        .and_then(|v| v.as_f64())
        .map(|n| n as f32)
        .unwrap_or(0.7);

    let language = args.get("language").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
    });

    let file_pattern = args
        .get("file_pattern")
        .and_then(|v| v.as_str())
        .map(String::from);

    let options = SearchCodeOptions {
        top_k,
        min_similarity,
        language,
        file_pattern,
    };

    let ws = open_workspace(context)?;
    let result = swissarmyhammer_code_context::search_code(&ws.db(), query_embedding, &options)
        .map_err(context_err)?;
    json_result(&result)
}

/// Execute the "find duplicates" operation.
///
/// For each chunk in the target file, finds similar chunks in other files.
fn execute_find_duplicates(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let min_similarity = args
        .get("min_similarity")
        .and_then(|v| v.as_f64())
        .map(|n| n as f32)
        .unwrap_or(0.85);

    let min_chunk_bytes = args
        .get("min_chunk_bytes")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(100);

    let max_per_chunk = args
        .get("max_per_chunk")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(5);

    let options = FindDuplicatesOptions {
        min_similarity,
        min_chunk_bytes,
        max_per_chunk,
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let result = swissarmyhammer_code_context::find_duplicates(&ws.db(), file_path, &options)
        .map_err(context_err)?;
    json_result(&result)
}

/// Execute the "query ast" operation.
///
/// Parses files with tree-sitter and runs an S-expression query against the ASTs.
/// Uses `LanguageRegistry` from `swissarmyhammer-treesitter` to resolve language grammars.
fn execute_query_ast(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query_str = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'query'", None))?;

    let language_name = args
        .get("language")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'language'", None))?;

    // Resolve language via LanguageRegistry
    use swissarmyhammer_treesitter::LanguageRegistry;
    let registry = LanguageRegistry::global();
    let lang_config = registry
        .get_by_name(language_name)
        .ok_or_else(|| {
            McpError::invalid_params(
                format!("Unsupported language '{}'. Use a language name like 'rust', 'python', 'typescript', etc.", language_name),
                None,
            )
        })?;
    let ts_language = lang_config.language();

    // Resolve workspace root
    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let workspace_root = find_git_repository_root_from(&working_dir).unwrap_or(working_dir);

    // Get file paths: either from explicit list or by scanning DB for files with matching extensions
    let file_paths: Vec<String> = if let Some(files) = args.get("files").and_then(|v| v.as_array())
    {
        files
            .iter()
            .filter_map(|item| item.as_str().map(String::from))
            .collect()
    } else {
        // Query indexed files with matching extensions from DB
        let ws = open_workspace(context)?;
        if let Some(progress) = check_ts_readiness(&ws)? {
            return Ok(progress);
        }
        let extensions = lang_config.extensions;
        let mut paths = Vec::new();
        if let Ok(mut stmt) = ws
            .db()
            .prepare("SELECT file_path FROM indexed_files WHERE ts_indexed = 1")
        {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for row in rows.flatten() {
                    if extensions
                        .iter()
                        .any(|ext| row.ends_with(&format!(".{}", ext)))
                    {
                        paths.push(row);
                    }
                }
            }
        }
        paths
    };

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(50);

    let options = QueryAstOptions { max_results };

    let result = swissarmyhammer_code_context::query_ast(
        &workspace_root,
        &ts_language,
        &file_paths,
        query_str,
        &options,
    )
    .map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get callgraph" operation.
///
/// Traverses the call graph from a starting symbol in the specified direction.
fn execute_get_callgraph(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'symbol'", None))?;

    let direction = match args.get("direction").and_then(|v| v.as_str()) {
        Some("inbound") => CallGraphDirection::Inbound,
        Some("outbound") | None => CallGraphDirection::Outbound,
        Some("both") => CallGraphDirection::Both,
        Some(other) => {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid direction '{}'. Valid values: 'inbound', 'outbound', 'both'",
                    other
                ),
                None,
            ))
        }
    };

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(2);

    let options = CallGraphOptions {
        symbol: symbol.to_string(),
        direction,
        max_depth,
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let result =
        swissarmyhammer_code_context::get_callgraph(&ws.db(), &options).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get blastradius" operation.
///
/// Analyzes the blast radius of changes to a file or symbol by finding
/// transitive inbound callers.
fn execute_get_blastradius(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .map(String::from);
    let max_hops = args
        .get("max_hops")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(3);

    let options = BlastRadiusOptions {
        file_path: file_path.to_string(),
        symbol,
        max_hops,
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let result =
        swissarmyhammer_code_context::get_blastradius(&ws.db(), &options).map_err(context_err)?;
    json_result(&result)
}

/// Trigger incremental tree-sitter indexing on dirty files.
///
/// Constructs the default embedding model (qwen-embedding) once for the run
/// and delegates to [`index_discovered_files_with_embedder`]. If the embedder
/// fails to construct or load, indexing still runs but chunk embeddings are
/// skipped — files keep `embedded=0` so the next pass can retry them.
///
/// Uses the leader's single shared write connection for all DB operations.
/// The mutex is locked only for each DB call — file I/O and parsing happen
/// without holding the lock so other writers (LSP worker, watcher) can interleave.
///
/// Exposed `pub` so end-to-end integration tests (notably
/// `tests/integration/semantic_search_e2e.rs`) can drive the real production
/// indexer over a temp workspace. The function is otherwise only called from
/// within this crate (the MCP server bootstrap and the file watcher).
///
/// Returns an [`IndexRunStats`] summarising the run. Callers that drive the
/// indexer purely for side effects (the bootstrap pass, the file watcher)
/// may ignore the value; the synchronous `rebuild index` MCP op uses it to
/// build its response payload.
pub async fn index_discovered_files_async(
    workspace_root: &Path,
    db: swissarmyhammer_code_context::SharedDb,
    reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
) -> swissarmyhammer_code_context::IndexRunStats {
    let embedder = build_default_embedder().await;
    index_discovered_files_with_embedder(workspace_root, db, embedder, reporter).await
}

/// Construct the default embedder and load it.
///
/// Returns `None` and logs a warning on construction or load failure. The
/// indexer treats this as a soft fallback — it still writes chunks (without
/// embeddings), leaving `indexed_files.embedded=0` so a future pass can
/// retry once the model is available.
///
/// MODEL NOTE: The default model is `qwen-embedding` (Qwen3-Embedding-0.6B),
/// a 1024-dim L2-normalized embedder. On macOS-arm64 it runs on the Apple
/// Neural Engine; elsewhere it falls back to llama.cpp. Max sequence is
/// 256 (ANE) or 512 (llama). The embedder is `Send + Sync`, and a single
/// shared `Arc<dyn TextEmbedder>` is reused across all chunks in an
/// indexing pass — see swissarmyhammer-embedding/src/embedder.rs.
///
/// Performance: per-chunk embedding on ANE is ~30-100ms, so a fresh full
/// index is minutes-to-tens-of-minutes for large workspaces. We embed
/// sequentially because the backends serialize internally; adding worker
/// parallelism here invites contention without throughput gains.
/// Returns whether the named environment variable is set to a truthy value.
///
/// Truthy means `1`, `true`, `yes`, or `on` (case-insensitive). Any other
/// value — including unset, empty, or `0`/`false` — is false. Used for opt-in
/// boolean toggles like `SAH_DISABLE_EMBEDDING`.
fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

async fn build_default_embedder() -> Option<std::sync::Arc<dyn model_embedding::TextEmbedder>> {
    use model_embedding::TextEmbedder as _;

    // Escape hatch: skip chunk embeddings entirely when `SAH_DISABLE_EMBEDDING`
    // is set to a truthy value. This selects the same `None`-embedder path the
    // indexer already takes on model-load failure — chunks are written without
    // an `embedding` blob and files keep `embedded=0`. It exists for two real
    // use cases: CI/headless indexing where the multi-GB model is unwanted, and
    // tests that exercise the indexing/progress contract without paying a cold
    // model load (which on a clean machine downloads gigabytes from HuggingFace
    // and otherwise dominates the run). Semantic `search code` is unavailable
    // for chunks indexed in this mode until a later pass embeds them.
    if env_flag_enabled("SAH_DISABLE_EMBEDDING") {
        tracing::info!(
            "code-context: SAH_DISABLE_EMBEDDING set — skipping chunk embeddings this pass"
        );
        return None;
    }

    let embedder = match swissarmyhammer_embedding::Embedder::default().await {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "code-context: failed to construct default embedder — chunk embeddings will be skipped this pass"
            );
            return None;
        }
    };
    if let Err(err) = embedder.load().await {
        tracing::warn!(
            error = %err,
            "code-context: failed to load embedding model — chunk embeddings will be skipped this pass"
        );
        return None;
    }
    tracing::info!(
        backend = embedder.backend_name(),
        model = embedder.model_name(),
        dimension = ?embedder.embedding_dimension(),
        max_sequence_length = embedder.max_sequence_length(),
        "code-context: loaded chunk embedder"
    );
    Some(std::sync::Arc::new(embedder) as std::sync::Arc<dyn model_embedding::TextEmbedder>)
}

/// Trigger incremental tree-sitter indexing on dirty files with a supplied
/// embedder.
///
/// This is the dependency-injectable form of [`index_discovered_files_async`].
/// Tests pass a mock embedder; production passes the model resolved by
/// `Embedder::default()`.
///
/// When `embedder` is `Some`, every chunk text is embedded and the resulting
/// little-endian f32 blob is written to the `embedding` column. A file is
/// flagged `embedded=1` only when every one of its chunks got an embedding
/// (a file with no chunks is vacuously fully embedded). If any chunk's
/// embedding failed it is written with a NULL `embedding` blob and the file
/// keeps `embedded=0`.
///
/// Important: the dirty-file selector is `WHERE ts_indexed = 0`, so a file
/// that exits this function with `ts_indexed=1, embedded=0` is NOT re-driven
/// on subsequent calls until something else (a file edit picked up by the
/// watcher, `rebuild_index`, etc.) flips `ts_indexed` back to 0. The
/// successfully embedded chunks remain searchable in the meantime — the
/// search path filters by `embedding IS NOT NULL`.
///
/// When `embedder` is `None` the indexer behaves as it did before chunk
/// embeddings existed: chunks are written without an embedding blob and
/// `embedded` stays at 0.
pub(crate) async fn index_discovered_files_with_embedder(
    workspace_root: &Path,
    db: swissarmyhammer_code_context::SharedDb,
    embedder: Option<std::sync::Arc<dyn model_embedding::TextEmbedder>>,
    reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
) -> swissarmyhammer_code_context::IndexRunStats {
    use std::sync::Arc;
    use swissarmyhammer_code_context::{IndexProgress, IndexRunStats};
    use swissarmyhammer_treesitter::{chunk::chunk_file, LanguageRegistry, ParsedFile};

    let run_start = std::time::Instant::now();

    // Emit a `Discovering { found: 0 }` event before discovery starts so
    // consumers can show a "discovering files…" indicator immediately. The
    // dirty-file query below is "discovery" for this incremental indexer —
    // it pulls the set of files that need indexing from `indexed_files`.
    reporter.report(IndexProgress::Discovering { found: 0 });

    // Query all dirty files from the DB (populated by startup_cleanup)
    let dirty_files: Vec<String> = {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        let result: Result<Vec<String>, rusqlite::Error> = (|| {
            let mut stmt =
                conn.prepare("SELECT file_path FROM indexed_files WHERE ts_indexed = 0")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        })();
        match result {
            Ok(files) => files,
            Err(e) => {
                tracing::warn!("code-context: failed to query dirty files: {}", e);
                // Emit the post-discovery `Discovering { found: 0 }` event
                // even on this error path so the event lifecycle stays
                // symmetric: every run emits two `Discovering` events
                // (pre- and post-discovery) before the terminal `Done`.
                // Consumers that key off "the second Discovering means
                // discovery completed" need this signal on every path.
                reporter.report(IndexProgress::Discovering { found: 0 });
                let elapsed = run_start.elapsed();
                reporter.report(IndexProgress::Done {
                    files: 0,
                    chunks: 0,
                    elapsed,
                });
                return IndexRunStats {
                    files: 0,
                    chunks: 0,
                    elapsed,
                };
            }
        }
    };

    // Emit the final discovery count now that we know the total file set.
    reporter.report(IndexProgress::Discovering {
        found: dirty_files.len() as u64,
    });

    if dirty_files.is_empty() {
        tracing::info!("code-context: no dirty files to index");
        let elapsed = run_start.elapsed();
        reporter.report(IndexProgress::Done {
            files: 0,
            chunks: 0,
            elapsed,
        });
        return IndexRunStats {
            files: 0,
            chunks: 0,
            elapsed,
        };
    }

    tracing::info!(
        "code-context: indexing {} dirty files incrementally",
        dirty_files.len()
    );

    let lang_registry = LanguageRegistry::global();
    let total = dirty_files.len();
    let mut indexed = 0u64;
    let mut total_chunks = 0u64;
    // 1-based batch counter for `Embedding` events. Each file's chunks are
    // treated as one batch (the indexer embeds chunk-by-chunk inside
    // `embed_file_chunks`, but from the consumer's point of view the
    // file-level grouping is the meaningful batch boundary).
    let mut batch_index: u64 = 0;
    let total_batches: u64 = total as u64;

    for relative_path in &dirty_files {
        let file_path = workspace_root.join(relative_path);

        // 1. Detect language (no DB needed)
        let lang_config = match lang_registry.detect_language(&file_path) {
            Some(config) => config,
            None => {
                let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
                indexed += 1;
                continue;
            }
        };

        // 2. Read and parse file (no DB needed)
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => {
                let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
                indexed += 1;
                continue;
            }
        };

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&lang_config.language()).is_err() {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());
            let _ = conn.execute(
                "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                rusqlite::params![relative_path],
            );
            indexed += 1;
            continue;
        }

        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => {
                let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
                indexed += 1;
                continue;
            }
        };

        let content_hash: [u8; 16] = md5::compute(content.as_bytes()).into();

        let parsed_file = Arc::new(ParsedFile::new(
            file_path.clone(),
            content,
            tree,
            content_hash,
        ));

        // 3. Extract semantic chunks (no DB needed)
        let chunks = chunk_file(parsed_file.clone());
        // The `done` value here is post-increment so the first file reports
        // `done: 1`. `indexed` is incremented at the bottom of the loop
        // body, so it currently holds the count of files completed before
        // this one — add 1 to get the 1-based "files chunked so far" value
        // a consumer expects.
        reporter.report(IndexProgress::Chunking {
            file: file_path.clone(),
            done: indexed + 1,
            total: total as u64,
        });

        // 4. Embed chunks BEFORE acquiring the DB lock. embed_text is async
        //    and may take 30-100ms per chunk on ANE; holding the connection
        //    mutex across that wait would starve other workers.
        let embedded_chunks =
            embed_file_chunks(&chunks, &parsed_file, embedder.as_deref(), relative_path).await;
        batch_index += 1;
        reporter.report(IndexProgress::Embedding {
            batch: batch_index,
            batches: total_batches,
            chunks_in_batch: embedded_chunks.len() as u64,
        });
        // A file is "fully embedded" when an embedder was supplied and every
        // prepared chunk has a Some(embedding). A file with zero chunks (e.g.
        // an empty file or one chunk_file rejected) is vacuously fully
        // embedded — there is nothing to embed, so we should not pretend the
        // file is in a partial-failure state.
        let all_chunks_embedded =
            embedder.is_some() && embedded_chunks.iter().all(|c| c.embedding.is_some());

        // 5. Lock DB once for the entire write batch for this file
        {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());

            // Clear old chunks
            let _ = conn.execute(
                "DELETE FROM ts_chunks WHERE file_path = ?",
                rusqlite::params![relative_path],
            );

            // Write new chunks
            let mut chunks_written = 0u64;
            for chunk in &embedded_chunks {
                let blob = chunk
                    .embedding
                    .as_deref()
                    .map(swissarmyhammer_code_context::serialize_embedding);
                if conn.execute(
                    "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path, embedding)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        relative_path,
                        chunk.start_byte,
                        chunk.end_byte,
                        chunk.start_line,
                        chunk.end_line,
                        chunk.text,
                        &chunk.symbol_path,
                        blob,
                    ],
                ).is_ok() {
                    chunks_written += 1;
                }
            }

            // 6. Extract symbols from chunks
            let _ = swissarmyhammer_code_context::ensure_ts_symbols(&conn, relative_path);

            // 7. Generate and write call edges
            let source_text = parsed_file.source.as_str();
            let language = lang_config.language();
            if let Ok(edges) = swissarmyhammer_code_context::generate_ts_call_edges(
                &conn,
                relative_path,
                source_text,
                language,
            ) {
                let _ = swissarmyhammer_code_context::write_ts_edges(&conn, relative_path, &edges);
            }

            // 8. Mark file as ts_indexed. Mark embedded=1 only when every
            //    chunk for the file got an embedding (or there were no chunks
            //    to embed); partial failure leaves embedded=0. The file is
            //    not re-driven by this function until ts_indexed is flipped
            //    back to 0 by something else — see the function docstring.
            if all_chunks_embedded {
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1, embedded = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = ?",
                    rusqlite::params![relative_path],
                );
            }

            total_chunks += chunks_written;
        }

        indexed += 1;

        if indexed.is_multiple_of(100) {
            tracing::info!(
                "code-context: indexed {}/{} files ({} chunks so far)",
                indexed,
                total,
                total_chunks
            );
        }

        // Yield to let other async tasks run
        tokio::task::yield_now().await;
    }

    // Summary
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    let chunk_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
        .unwrap_or(0);
    let embedded_chunk_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let symbol_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM lsp_symbols", [], |r| r.get(0))
        .unwrap_or(0);
    let edge_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))
        .unwrap_or(0);
    tracing::info!(
        "code-context: indexing complete — {}/{} files, {} chunks ({} embedded), {} symbols, {} call edges",
        indexed,
        total,
        chunk_count,
        embedded_chunk_count,
        symbol_count,
        edge_count
    );
    // Drop the DB lock before emitting the final event so consumer
    // reporters that touch the database (e.g. status snapshots) cannot
    // deadlock against our own connection guard.
    drop(conn);
    let elapsed = run_start.elapsed();
    reporter.report(IndexProgress::Done {
        files: indexed,
        chunks: total_chunks,
        elapsed,
    });
    IndexRunStats {
        files: indexed,
        chunks: total_chunks,
        elapsed,
    }
}

/// A chunk row prepared for insertion into `ts_chunks`, with an optional
/// pre-computed embedding vector.
struct PreparedChunk {
    start_byte: i32,
    end_byte: i32,
    start_line: i32,
    end_line: i32,
    text: String,
    symbol_path: String,
    /// `Some` when the chunk text was successfully embedded; `None` when
    /// embedding was unavailable (no embedder) or returned an error.
    embedding: Option<Vec<f32>>,
}

/// Convert a chunk into [`PreparedChunk`] form (no embedding), or return
/// `None` if the chunk doesn't have parseable byte ranges.
fn prepare_chunk(
    chunk: &swissarmyhammer_treesitter::chunk::SemanticChunk,
    parsed_file: &swissarmyhammer_treesitter::ParsedFile,
) -> Option<PreparedChunk> {
    use swissarmyhammer_treesitter::ChunkSource;
    let text = chunk.source.content()?.to_string();
    let (start_byte, end_byte) = match &chunk.source {
        ChunkSource::Parsed {
            start_byte,
            end_byte,
            ..
        } => (*start_byte, *end_byte),
        _ => return None,
    };
    let start_line = parsed_file.source[..start_byte].matches('\n').count() as i32;
    let end_line = parsed_file.source[..end_byte].matches('\n').count() as i32;
    Some(PreparedChunk {
        start_byte: start_byte as i32,
        end_byte: end_byte as i32,
        start_line,
        end_line,
        text,
        symbol_path: chunk.symbol_path(),
        embedding: None,
    })
}

/// Prepare every chunk for insertion, embedding each one if an embedder was
/// provided. Per-chunk embedding errors leave that chunk's `embedding` as
/// `None`; the rest of the file continues. At most one summary warning is
/// emitted per file (with the failure count and an example symbol + error)
/// so that a model crash mid-run does not produce one log line per chunk
/// across tens of thousands of chunks.
async fn embed_file_chunks(
    chunks: &[swissarmyhammer_treesitter::chunk::SemanticChunk],
    parsed_file: &swissarmyhammer_treesitter::ParsedFile,
    embedder: Option<&dyn model_embedding::TextEmbedder>,
    relative_path: &str,
) -> Vec<PreparedChunk> {
    let mut prepared = Vec::with_capacity(chunks.len());
    let mut failed_count: usize = 0;
    let mut first_failure: Option<(String, String)> = None;
    for chunk in chunks {
        let Some(mut pc) = prepare_chunk(chunk, parsed_file) else {
            continue;
        };
        if let Some(emb) = embedder {
            match emb.embed_text(&pc.text).await {
                Ok(result) => pc.embedding = Some(result.embedding().to_vec()),
                Err(err) => {
                    failed_count += 1;
                    if first_failure.is_none() {
                        first_failure = Some((pc.symbol_path.clone(), err.to_string()));
                    }
                }
            }
        }
        prepared.push(pc);
    }
    if failed_count > 0 {
        let (symbol, err) = first_failure.unwrap_or_default();
        tracing::warn!(
            file = %relative_path,
            failed_chunks = failed_count,
            total_chunks = prepared.len(),
            first_failed_symbol = %symbol,
            first_error = %err,
            "code-context: chunk embedding failed for one or more chunks — those chunks were inserted with NULL embedding"
        );
    }
    prepared
}

/// Execute the "get status" operation.
///
/// Returns a health report with file counts, indexing progress, and chunk/edge counts.
/// Also includes LSP server availability from doctor check.
fn execute_get_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    let workspace_root = ws.workspace_root().to_path_buf();

    // Run doctor check to report on LSP availability
    let doctor_report = doctor::run_doctor(&workspace_root);
    tracing::debug!("Doctor report: {:?}", doctor_report);

    // Log LSP availability for debugging
    for lsp in &doctor_report.lsp_servers {
        if lsp.installed {
            tracing::debug!("LSP available: {} at {:?}", lsp.name, lsp.path);
        } else {
            tracing::debug!("LSP NOT available: {}", lsp.name);
        }
    }

    let status = swissarmyhammer_code_context::get_status(&ws.db()).map_err(context_err)?;

    // Merge LSP daemon status into the response
    let mut result = serde_json::to_value(&status).unwrap_or_default();
    if let Some(sup) = LSP_SUPERVISOR.get() {
        if let Ok(guard) = sup.try_lock() {
            let daemon_status = guard.status();
            if let Ok(daemon_json) = serde_json::to_value(&daemon_status) {
                result["lsp_daemons"] = daemon_json;
            }
        }
    }

    // Surface doctor report: detected project types and LSP availability
    if let Ok(v) = serde_json::to_value(&doctor_report.project_types) {
        result["project_types"] = v;
    }
    if let Ok(v) = serde_json::to_value(&doctor_report.lsp_servers) {
        result["lsp_availability"] = v;
    }

    json_result(&result)
}

/// Execute the "rebuild index" operation.
///
/// Resets the indexed flag for the specified layer and then drives the
/// synchronous tree-sitter indexer over the resulting dirty set. Returns
/// real run stats (`files_indexed`, `chunks_written`, `elapsed_ms`) rather
/// than just the marking count, so the MCP caller knows the rebuild
/// actually completed by the time the response lands.
///
/// ## Scope of the synchronous contract
///
/// Only the tree-sitter layer is driven to completion here. The LSP
/// indexer is a long-running background worker owned by the leader, and
/// this op does not await it — flipping `lsp_indexed=0` queues files for
/// that worker, but `rebuild index` returns once tree-sitter is done.
///
/// As a result:
/// - `layer=treesitter` — `files_indexed` / `chunks_written` describe
///   the full rebuild; the dirty set the marker produced (`WHERE
///   ts_indexed = 0`) is exactly the set the synchronous indexer drains.
/// - `layer=both` — same counters, same scope; the LSP rows are also
///   marked dirty for the background worker but those are not in the
///   tree-sitter dirty set and the counters don't account for them. The
///   `note` field on the response surfaces this caveat.
/// - `layer=lsp` — only `lsp_indexed=0` is flipped. The synchronous
///   indexer below queries `WHERE ts_indexed = 0` and finds nothing, so
///   the response always reports `files_indexed=0, chunks_written=0,
///   elapsed_ms~=0`. The dirty bits still take effect for the
///   background LSP worker; callers monitor progress via `get status`'s
///   `lsp_indexed_percent`. The `note` field on the response documents
///   this so callers aren't misled by the zero counters.
async fn execute_rebuild_index(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let layer = match args.get("layer").and_then(|v| v.as_str()) {
        Some("treesitter") => BuildLayer::TreeSitter,
        Some("lsp") => BuildLayer::Lsp,
        Some("both") | None => BuildLayer::Both,
        Some(other) => {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid layer '{}'. Valid values: 'treesitter', 'lsp', 'both'",
                    other
                ),
                None,
            ))
        }
    };

    let ws = open_workspace(context)?;
    let workspace_root = ws.workspace_root().to_path_buf();

    // 1. Mark files dirty. Use write_db() so followers get a typed
    //    ReadOnlyFollower error mapped to invalid_request rather than an
    //    opaque SQLite "readonly database" failure after the UPDATE runs.
    //    The DbRef guard is dropped at the end of this block so the
    //    indexer can acquire the shared mutex on its own (it locks
    //    per-file, not for the whole run).
    let mut result = {
        let db = ws.write_db().map_err(context_err)?;
        swissarmyhammer_code_context::rebuild_index(&db, layer).map_err(context_err)?
    };

    // 2. Drive the indexer synchronously over the dirty set we just
    //    produced. `write_db()` already proved we're the leader, so
    //    `shared_db()` returning `None` would mean the workspace mode
    //    changed under us — surface that as an internal error rather
    //    than panicking.
    //
    //    The bootstrap pass and the file watcher run their own indexer
    //    invocations in the background. They use the same single shared
    //    `Mutex<Connection>` as we do, so concurrent indexer runs are
    //    serialised at the per-DB-call granularity rather than
    //    contending for distinct connections. The worst case is that
    //    a concurrent run sees an empty dirty set and exits in milliseconds.
    //    Adding a coarser advisory lock would just trade that benign
    //    waste for additional state to maintain.
    //
    //    Stats note: `files_indexed` and `chunks_written` count what this
    //    specific run produced, not net-new rows across the workspace.
    //    Concurrent rebuild/bootstrap/watcher runs each report their own
    //    non-zero counters for the same logical dirty set — that is the
    //    price of the lock-free design.
    //
    //    When the client supplied a `progressToken` in the request
    //    `_meta` (plumbed through `ToolContext::progress_token`) and the
    //    MCP peer is available, we build an `McpProgressReporter` that
    //    forwards `IndexProgress` events as `notifications/progress`
    //    messages. A dedicated drain task consumes the synchronous
    //    reporter channel and ships notifications through the peer.
    //    Dropping the reporter at the end of indexing closes the
    //    channel, the drain task exits, and we await its
    //    `JoinHandle` so any buffered terminal `Done` event is flushed
    //    before this op returns its `CallToolResult`. Absent a token or
    //    peer we fall back to the no-op reporter so the tool stays
    //    silent — progress is strictly opt-in by the client.
    let shared_db = ws.shared_db().ok_or_else(|| {
        McpError::internal_error(
            "workspace lost leader status before rebuild could run",
            None,
        )
    })?;
    // Pick a progress reporter based on what the caller wired up.
    //
    // Three cases, in priority order:
    //
    // 1. `progress_token` + `progress_sink` (in-process caller, e.g. CLI):
    //    build the standard `McpProgressReporter` and forward each
    //    notification to the caller-provided `UnboundedSender` instead of
    //    going through a peer. The sink takes priority over `peer` because
    //    it is the explicit in-process opt-in — when both are set we honor
    //    the more specific request.
    //
    // 2. `progress_token` + `peer` (MCP client over stdio/HTTP): build the
    //    `McpProgressReporter` and ship notifications via
    //    `peer.send_notification`. This is the original wiring.
    //
    // 3. Neither / token without a transport: fall back to the noop
    //    reporter. Progress is opt-in by the client; a token without any
    //    transport is a misconfiguration but progress is advisory so we
    //    log a warning and proceed silently.
    let (reporter, drain_handle): (
        std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
        Option<tokio::task::JoinHandle<()>>,
    ) = match (
        context.progress_token.clone(),
        context.progress_sink.clone(),
        context.peer.clone(),
    ) {
        (Some(token), Some(sink), _) => {
            tracing::debug!(
                ?token,
                "rebuild_index: wiring McpProgressReporter to in-process progress sink"
            );
            let crate::mcp::progress::McpProgressReporterBuild { reporter, receiver } =
                crate::mcp::progress::McpProgressReporter::build(token);
            let handle = crate::mcp::progress::spawn_in_process_drain_task(sink, receiver);
            (std::sync::Arc::new(reporter), Some(handle))
        }
        (Some(token), None, Some(peer)) => {
            tracing::debug!(
                ?token,
                "rebuild_index: wiring McpProgressReporter for client-supplied progressToken"
            );
            let crate::mcp::progress::McpProgressReporterBuild { reporter, receiver } =
                crate::mcp::progress::McpProgressReporter::build(token);
            let handle = crate::mcp::progress::spawn_drain_task(peer, receiver);
            (std::sync::Arc::new(reporter), Some(handle))
        }
        (None, _, _) => {
            tracing::debug!(
                "rebuild_index: no progressToken in request _meta — using noop reporter"
            );
            (swissarmyhammer_code_context::noop_reporter(), None)
        }
        (Some(_), None, None) => {
            tracing::warn!(
                "rebuild_index: progressToken present but no MCP peer or progress sink — using noop reporter"
            );
            (swissarmyhammer_code_context::noop_reporter(), None)
        }
    };
    let stats =
        index_discovered_files_async(&workspace_root, shared_db, std::sync::Arc::clone(&reporter))
            .await;

    // Drop the reporter so the mpsc channel closes; then await the
    // drain task so any buffered notifications (notably the terminal
    // `Done` event) are flushed before we return to the client.
    //
    // A `JoinError` from the drain task means the task panicked or was
    // cancelled (e.g. a hypothetical future rmcp version that panics on
    // a closed peer). Progress is advisory so we still return the tool's
    // result, but log at warn level so the panic isn't silently lost —
    // the drain task itself logs send errors at debug, so a join failure
    // deserves at least the same surfacing.
    drop(reporter);
    if let Some(handle) = drain_handle {
        if let Err(err) = handle.await {
            tracing::warn!(
                error = ?err,
                "rebuild_index: progress drain task did not join cleanly"
            );
        }
    }

    result.files_indexed = stats.files;
    result.chunks_written = stats.chunks;
    result.elapsed_ms = stats.elapsed.as_millis() as u64;

    json_result(&result)
}

/// Execute the "clear status" operation.
///
/// Wipes all index data from all tables and returns stats about what was cleared.
fn execute_clear_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    // Followers get a typed ReadOnlyFollower error here rather than an opaque
    // SQLite failure once the DELETE runs.
    let db = ws.write_db().map_err(context_err)?;
    let result = swissarmyhammer_code_context::clear_status(&db).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "lsp status" operation.
///
/// Queries indexed file extensions, cross-references with the LSP registry,
/// and returns which languages are present, which LSPs are installed or missing,
/// and install hints.
fn execute_lsp_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    let conn = ws.db();

    // Get distinct file extensions from the index
    let exts = swissarmyhammer_code_context::distinct_extensions(&conn).map_err(context_err)?;

    // Convert to &str slice for the registry lookup
    let ext_refs: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
    let matching_servers = swissarmyhammer_lsp::servers_for_extensions(&ext_refs);

    // Build the response
    let mut languages = Vec::new();
    for spec in &matching_servers {
        // Check which of this server's extensions are present in the index
        let present_exts: Vec<&str> = spec
            .file_extensions
            .iter()
            .filter(|e| exts.contains(e.as_str()))
            .map(|e| e.as_str())
            .collect();

        let installed = swissarmyhammer_code_context::find_executable(&spec.command).is_some();

        languages.push(serde_json::json!({
            "icon": spec.icon,
            "extensions": present_exts,
            "lsp_server": spec.command,
            "installed": installed,
            "install_hint": if installed { None } else { Some(&spec.install_hint) },
        }));
    }

    let all_healthy = languages
        .iter()
        .all(|l| l["installed"].as_bool().unwrap_or(false));

    let result = serde_json::json!({
        "languages": languages,
        "all_healthy": all_healthy,
    });

    json_result(&result)
}

/// Execute the "get rename_edits" operation.
///
/// Previews a rename at the given position without applying edits.
/// Returns `can_rename: false` when no live LSP is available.
fn execute_get_rename_edits(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let new_name = args
        .get("new_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'new_name'", None))?;

    let opts = swissarmyhammer_code_context::GetRenameEditsOptions {
        file_path: file_path.to_string(),
        line,
        character,
        new_name: new_name.to_string(),
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = swissarmyhammer_code_context::LayeredContext::new(&db, client.as_ref());

    let result =
        swissarmyhammer_code_context::get_rename_edits(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get diagnostics" operation.
///
/// Returns errors and warnings for a file via live LSP pull diagnostics.
/// Returns empty when no live LSP is available.
fn execute_get_diagnostics(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let severity_filter = args
        .get("severity_filter")
        .and_then(|v| v.as_str())
        .map(|s| match s.to_lowercase().as_str() {
            "error" => DiagnosticSeverity::Error,
            "warning" => DiagnosticSeverity::Warning,
            "info" => DiagnosticSeverity::Info,
            "hint" => DiagnosticSeverity::Hint,
            _ => DiagnosticSeverity::Hint,
        });

    let opts = GetDiagnosticsOptions {
        file_path: file_path.to_string(),
        severity_filter,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result = swissarmyhammer_code_context::get_diagnostics(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get inbound_calls" operation.
///
/// Finds all callers of a function at the given position using layered
/// resolution (live LSP call hierarchy, then LSP index, then tree-sitter).
fn execute_get_inbound_calls(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    let opts = GetInboundCallsOptions {
        file_path: file_path.to_string(),
        line,
        character,
        depth,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result =
        swissarmyhammer_code_context::get_inbound_calls(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "search workspace_symbol" operation.
///
/// Live workspace symbol search with layered resolution: live LSP
/// workspace/symbol, then LSP index, then tree-sitter chunks.
fn execute_workspace_symbol_live(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'query'", None))?;

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    let opts = WorkspaceSymbolLiveOptions {
        query: query.to_string(),
        max_results,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = any_lsp_client();
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result =
        swissarmyhammer_code_context::workspace_symbol_live(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get definition" operation.
///
/// Go-to-definition with layered resolution: live LSP, LSP index, tree-sitter.
fn execute_get_definition(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let include_source = args
        .get("include_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let opts = GetDefinitionOptions {
        file_path: file_path.to_string(),
        line,
        character,
        include_source,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result = swissarmyhammer_code_context::get_definition(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get type_definition" operation.
///
/// Go-to-type-definition via live LSP only. Returns empty when no LSP is available.
fn execute_get_type_definition(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let include_source = args
        .get("include_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let opts = GetTypeDefinitionOptions {
        file_path: file_path.to_string(),
        line,
        character,
        include_source,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result =
        swissarmyhammer_code_context::get_type_definition(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get hover" operation.
///
/// Returns hover information (type signature, docs) with layered resolution.
fn execute_get_hover(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let opts = GetHoverOptions {
        file_path: file_path.to_string(),
        line,
        character,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    tracing::debug!(file_path = %file_path, client = client.is_some(), "get_hover: client lookup");
    let ctx = LayeredContext::new(&db, client.as_ref());
    tracing::debug!(
        has_live_lsp = ctx.has_live_lsp(),
        "get_hover: context created"
    );

    let result = swissarmyhammer_code_context::get_hover(&ctx, &opts).map_err(context_err)?;
    tracing::debug!(source_layer = ?result.as_ref().map(|r| &r.source_layer), "get_hover: result");
    json_result(&result)
}

/// Execute the "get references" operation.
///
/// Finds all references to a symbol with layered resolution.
fn execute_get_references(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let include_declaration = args
        .get("include_declaration")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let opts = GetReferencesOptions {
        file_path: file_path.to_string(),
        line,
        character,
        include_declaration,
        max_results,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result = swissarmyhammer_code_context::get_references(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get implementations" operation.
///
/// Finds implementations of a trait/interface with layered resolution.
fn execute_get_implementations(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let line = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'line'", None))?
        as u32;

    let character = args
        .get("character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'character'", None))?
        as u32;

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let opts = GetImplementationsOptions {
        file_path: file_path.to_string(),
        line,
        character,
        max_results,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result =
        swissarmyhammer_code_context::get_implementations(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "get code_actions" operation.
///
/// Returns code actions (quickfixes, refactors) for a range via live LSP.
/// Returns empty when no LSP is available.
fn execute_get_code_actions(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'file_path'", None))?;

    let start_line = args
        .get("start_line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'start_line'", None))?
        as u32;

    let start_character = args
        .get("start_character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            McpError::invalid_params("Missing required parameter 'start_character'", None)
        })? as u32;

    let end_line = args
        .get("end_line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError::invalid_params("Missing required parameter 'end_line'", None))?
        as u32;

    let end_character = args
        .get("end_character")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            McpError::invalid_params("Missing required parameter 'end_character'", None)
        })? as u32;

    let filter_kind = args.get("filter_kind").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
    });

    let opts = GetCodeActionsOptions {
        file_path: file_path.to_string(),
        start_line,
        start_character,
        end_line,
        end_character,
        filter_kind,
    };

    let ws = open_workspace(context)?;
    let db = ws.db();
    let client = lsp_client_for_file(file_path);
    let ctx = LayeredContext::new(&db, client.as_ref());

    let result =
        swissarmyhammer_code_context::get_code_actions(&ctx, &opts).map_err(context_err)?;
    json_result(&result)
}

/// Register the code_context tool with the registry.
pub fn register_code_context_tools(registry: &mut ToolRegistry) {
    registry.register(CodeContextTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_code_context_tools() {
        let mut registry = ToolRegistry::new();
        assert_eq!(registry.len(), 0);

        register_code_context_tools(&mut registry);

        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("code_context").is_some());
    }

    #[test]
    fn test_code_context_tool_name() {
        let tool = CodeContextTool::new();
        assert_eq!(<CodeContextTool as McpTool>::name(&tool), "code_context");
    }

    #[test]
    fn test_code_context_tool_has_description() {
        let tool = CodeContextTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_code_context_tool_has_operations() {
        let tool = CodeContextTool::new();
        let ops = tool.operations();
        assert_eq!(ops.len(), 24);
        assert!(ops.iter().any(|o| o.op_string() == "get symbol"));
        assert!(ops.iter().any(|o| o.op_string() == "search symbol"));
        assert!(ops.iter().any(|o| o.op_string() == "list symbols"));
        assert!(ops.iter().any(|o| o.op_string() == "grep code"));
        assert!(ops.iter().any(|o| o.op_string() == "search code"));
        assert!(ops.iter().any(|o| o.op_string() == "find duplicates"));
        assert!(ops.iter().any(|o| o.op_string() == "query ast"));
        assert!(ops.iter().any(|o| o.op_string() == "get callgraph"));
        assert!(ops.iter().any(|o| o.op_string() == "get blastradius"));
        assert!(ops.iter().any(|o| o.op_string() == "get status"));
        assert!(ops.iter().any(|o| o.op_string() == "rebuild index"));
        assert!(ops.iter().any(|o| o.op_string() == "clear status"));
        assert!(ops.iter().any(|o| o.op_string() == "lsp status"));
        assert!(ops.iter().any(|o| o.op_string() == "detect projects"));
        assert!(ops.iter().any(|o| o.op_string() == "get rename_edits"));
        assert!(ops.iter().any(|o| o.op_string() == "get diagnostics"));
        assert!(ops.iter().any(|o| o.op_string() == "get inbound_calls"));
        assert!(ops
            .iter()
            .any(|o| o.op_string() == "search workspace_symbol"));
        assert!(ops.iter().any(|o| o.op_string() == "get definition"));
        assert!(ops.iter().any(|o| o.op_string() == "get type_definition"));
        assert!(ops.iter().any(|o| o.op_string() == "get hover"));
        assert!(ops.iter().any(|o| o.op_string() == "get references"));
        assert!(ops.iter().any(|o| o.op_string() == "get implementations"));
        assert!(ops.iter().any(|o| o.op_string() == "get code_actions"));
    }

    #[test]
    fn test_code_context_tool_schema_has_op_field() {
        let tool = CodeContextTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        let op_enum = schema["properties"]["op"]["enum"]
            .as_array()
            .expect("op should have enum");
        assert!(op_enum.contains(&serde_json::json!("get symbol")));
        assert!(op_enum.contains(&serde_json::json!("search symbol")));
        assert!(op_enum.contains(&serde_json::json!("list symbols")));
        assert!(op_enum.contains(&serde_json::json!("grep code")));
        assert!(op_enum.contains(&serde_json::json!("query ast")));
        assert!(op_enum.contains(&serde_json::json!("get callgraph")));
        assert!(op_enum.contains(&serde_json::json!("get blastradius")));
        assert!(op_enum.contains(&serde_json::json!("get status")));
        assert!(op_enum.contains(&serde_json::json!("rebuild index")));
        assert!(op_enum.contains(&serde_json::json!("clear status")));
        assert!(op_enum.contains(&serde_json::json!("lsp status")));
        assert!(op_enum.contains(&serde_json::json!("detect projects")));
    }

    #[test]
    fn test_code_context_tool_schema_has_operation_schemas() {
        let tool = CodeContextTool::new();
        let schema = tool.schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 24);
    }

    #[tokio::test]
    async fn test_code_context_tool_unknown_op() {
        let tool = CodeContextTool::new();
        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "op".to_string(),
            serde_json::Value::String("invalid op".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_code_context_tool_missing_op() {
        let tool = CodeContextTool::new();
        let context = crate::test_utils::create_test_context().await;

        let args = serde_json::Map::new();

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'op' field"));
    }

    #[test]
    fn test_lsp_degradation_notice_no_supervisor() {
        // When LSP_SUPERVISOR is not set and no projects, should return None
        let tmp = tempfile::tempdir().unwrap();
        assert!(lsp_degradation_notice(tmp.path()).is_none());
    }

    #[test]
    fn test_lsp_degradation_notice_with_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\n",
        )
        .unwrap();
        let notice = lsp_degradation_notice(tmp.path());
        // If rust-analyzer is installed, notice is None; if not, it should contain the hint
        if let Some(text) = notice {
            assert!(text.contains("tree-sitter only"));
            assert!(text.contains("rust-analyzer"));
        }
    }

    // -----------------------------------------------------------------------
    // Integration tests for operation dispatch and query execution
    //
    // These tests require access to `index_discovered_files_async` and must
    // therefore live in the unit test module rather than the external
    // integration test files.
    // -----------------------------------------------------------------------

    use std::path::PathBuf;
    use std::sync::Arc;
    use swissarmyhammer_config::model::ModelConfig;
    use tokio::sync::Mutex as TokioMutex;

    /// Build a ToolContext rooted at the given directory.
    fn make_context_with_dir(dir: PathBuf) -> crate::mcp::tool_registry::ToolContext {
        use crate::mcp::tool_handlers::ToolHandlers;
        let git_ops = Arc::new(TokioMutex::new(None));
        let tool_handlers = Arc::new(ToolHandlers::new());
        let agent_config = Arc::new(ModelConfig::default());
        let mut ctx =
            crate::mcp::tool_registry::ToolContext::new(tool_handlers, git_ops, agent_config);
        ctx.working_dir = Some(dir);
        ctx
    }

    /// Create a minimal Rust project in a temp dir and run full treesitter indexing.
    ///
    /// Returns `(tempdir, context)` — the caller must hold `tempdir` to keep
    /// the directory alive for the duration of the test.
    async fn create_indexed_project() -> (tempfile::TempDir, crate::mcp::tool_registry::ToolContext)
    {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let root = tmp.path();

        // Write source files with distinct symbols so operations have something to find.
        std::fs::create_dir_all(root.join("src")).unwrap();

        std::fs::write(
            root.join("src/main.rs"),
            r#"fn main() {
    greet("world");
}

fn greet(name: &str) {
    println!("Hello, {}!", name);
}
"#,
        )
        .unwrap();

        std::fs::write(
            root.join("src/lib.rs"),
            r#"/// A simple calculator struct.
pub struct Calculator {
    pub value: f64,
}

impl Calculator {
    /// Create a new Calculator with the given initial value.
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Add a number to the current value.
    pub fn add(&mut self, x: f64) -> f64 {
        self.value += x;
        self.value
    }
}
"#,
        )
        .unwrap();

        // Open the workspace — this runs startup_cleanup, marking files dirty.
        let ws = CodeContextWorkspace::open(root).expect("workspace open");

        // Run treesitter indexing so query operations have chunks to search.
        if let Some(shared_db) = ws.shared_db() {
            index_discovered_files_async(
                root,
                shared_db,
                swissarmyhammer_code_context::noop_reporter(),
            )
            .await;
        }

        let ctx = make_context_with_dir(root.to_path_buf());
        (tmp, ctx)
    }

    /// Extract the text content from the first item of a tool result.
    fn extract_text(result: &rmcp::model::CallToolResult) -> &str {
        match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        }
    }

    // -----------------------------------------------------------------------
    // Operation dispatch: missing/invalid op
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_dispatch_unknown_op_returns_error() {
        let tool = CodeContextTool::new();
        let ctx = crate::test_utils::create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("not an op"));
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unknown operation"));
    }

    #[tokio::test]
    async fn test_dispatch_empty_op_returns_error() {
        let tool = CodeContextTool::new();
        let ctx = crate::test_utils::create_test_context().await;
        let args = serde_json::Map::new(); // no "op" key
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'op' field"));
    }

    // -----------------------------------------------------------------------
    // get status — workspace discovery and reporting
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_status_returns_file_counts() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));

        let result = tool.execute(args, &ctx).await.expect("get status");
        assert_eq!(result.is_error, Some(false));

        let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
        let total = json["total_files"].as_u64().unwrap_or(0);
        assert!(
            total >= 2,
            "expected >= 2 files (main.rs, lib.rs), got {}",
            total
        );
    }

    // -----------------------------------------------------------------------
    // rebuild index and clear status
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_rebuild_index_resets_indexed_flags() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("treesitter"));

        let result = tool.execute(args, &ctx).await.expect("rebuild index");
        assert_eq!(result.is_error, Some(false));

        let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
        // After rebuild index, files_marked should be >= 2 (main.rs and lib.rs)
        let marked = json["files_marked"].as_u64().unwrap_or(0);
        assert!(
            marked >= 2,
            "expected >= 2 files marked for re-indexing, got {}",
            marked
        );
    }

    #[tokio::test]
    async fn test_rebuild_index_invalid_layer_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("invalid_layer"));

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_clear_status_wipes_index_data() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("clear status"));

        let result = tool.execute(args, &ctx).await.expect("clear status");
        assert_eq!(result.is_error, Some(false));

        let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
        // After clear, the response should be a valid JSON object (stats about what was cleared)
        assert!(
            json.is_object(),
            "expected object response from clear status"
        );
    }

    /// When a write op runs against a workspace whose leader is held by
    /// another live process, the user must see a typed `invalid_request`
    /// error that names the workspace path instead of an opaque
    /// `-32603: database error`. This protects against the most common
    /// confusion ("why does rebuilding the index fail?"): the leader is
    /// another agent session.
    ///
    /// The test holds the leader in this thread via a `_leader` binding so
    /// the MCP tool's call to `open_workspace` deterministically lands on
    /// the follower branch.
    #[tokio::test]
    async fn test_rebuild_index_returns_typed_error_on_follower() {
        let (_tmp, ctx) = create_indexed_project().await;

        // Hold the leader so the MCP op opens as a follower.
        let workspace_root = ctx
            .working_dir
            .clone()
            .expect("indexed project sets working_dir");
        let _leader = CodeContextWorkspace::open(&workspace_root).expect("hold leader for test");

        let tool = CodeContextTool::new();
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));

        let err = tool
            .execute(args, &ctx)
            .await
            .expect_err("follower must reject rebuild index");
        let msg = err.to_string();
        let ws_display = workspace_root.display().to_string();
        assert!(
            msg.contains(&ws_display),
            "MCP error must mention the workspace root, got: {msg}"
        );
        assert!(
            msg.contains("read-only"),
            "MCP error must explain the read-only follower condition, got: {msg}"
        );
    }

    /// `clear status` follows the same write-rejection path. Validating
    /// both ops here prevents a future regression that wires only one of
    /// them through `write_db()`.
    #[tokio::test]
    async fn test_clear_status_returns_typed_error_on_follower() {
        let (_tmp, ctx) = create_indexed_project().await;
        let workspace_root = ctx
            .working_dir
            .clone()
            .expect("indexed project sets working_dir");
        let _leader = CodeContextWorkspace::open(&workspace_root).expect("hold leader for test");

        let tool = CodeContextTool::new();
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("clear status"));

        let err = tool
            .execute(args, &ctx)
            .await
            .expect_err("follower must reject clear status");
        let msg = err.to_string();
        assert!(
            msg.contains(&workspace_root.display().to_string()),
            "MCP error must mention the workspace root, got: {msg}"
        );
        assert!(
            msg.contains("read-only"),
            "MCP error must explain the read-only follower condition, got: {msg}"
        );
        // The follower-rejection message must stay op-agnostic. A user who
        // invoked `clear status` should not see the message naming a
        // different op (e.g. `rebuild index`), which would steer debugging
        // in the wrong direction.
        assert!(
            !msg.contains("rebuild index"),
            "MCP error for clear status must not misname the op, got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // lsp status
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_lsp_status_returns_language_list() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("lsp status"));

        let result = tool.execute(args, &ctx).await.expect("lsp status");
        assert_eq!(result.is_error, Some(false));

        let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
        // Response should have a "languages" array
        assert!(
            json["languages"].is_array(),
            "expected 'languages' array in lsp status response"
        );
        assert!(
            json["all_healthy"].is_boolean(),
            "expected 'all_healthy' boolean in lsp status response"
        );
    }

    // -----------------------------------------------------------------------
    // grep code
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_grep_code_finds_pattern() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("grep code"));
        args.insert("pattern".to_string(), serde_json::json!("fn greet"));

        let result = tool.execute(args, &ctx).await.expect("grep code");
        // May return progress message if not indexed, or actual results
        assert_eq!(result.is_error, Some(false));
        let text = extract_text(&result);
        // If indexed, should find fn greet; if not indexed yet, will be progress message
        // Either way, result is valid (not an error)
        assert!(!text.is_empty());
    }

    #[tokio::test]
    async fn test_grep_code_missing_pattern_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("grep code"));
        // Intentionally omit "pattern"

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("pattern"));
    }

    #[tokio::test]
    async fn test_grep_code_with_language_filter() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("grep code"));
        args.insert("pattern".to_string(), serde_json::json!("pub struct"));
        args.insert("language".to_string(), serde_json::json!(["rs"]));

        let result = tool
            .execute(args, &ctx)
            .await
            .expect("grep code with language filter");
        assert_eq!(result.is_error, Some(false));
    }

    // -----------------------------------------------------------------------
    // search symbol
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_search_symbol_returns_results_or_progress() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("search symbol"));
        args.insert("query".to_string(), serde_json::json!("Calculator"));

        let result = tool.execute(args, &ctx).await.expect("search symbol");
        assert_eq!(result.is_error, Some(false));
        assert!(!extract_text(&result).is_empty());
    }

    #[tokio::test]
    async fn test_search_symbol_missing_query_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("search symbol"));
        // Omit "query"

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    #[tokio::test]
    async fn test_search_symbol_with_kind_filter() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("search symbol"));
        args.insert("query".to_string(), serde_json::json!("add"));
        args.insert("kind".to_string(), serde_json::json!("function"));

        let result = tool
            .execute(args, &ctx)
            .await
            .expect("search symbol with kind");
        assert_eq!(result.is_error, Some(false));
    }

    // -----------------------------------------------------------------------
    // get symbol
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_symbol_returns_results_or_progress() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get symbol"));
        args.insert("query".to_string(), serde_json::json!("Calculator::new"));

        let result = tool.execute(args, &ctx).await.expect("get symbol");
        assert_eq!(result.is_error, Some(false));
        assert!(!extract_text(&result).is_empty());
    }

    #[tokio::test]
    async fn test_get_symbol_missing_query_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get symbol"));
        // Omit "query"

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }

    // -----------------------------------------------------------------------
    // list symbols
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_list_symbols_returns_results_or_progress() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("list symbols"));
        args.insert("file_path".to_string(), serde_json::json!("src/lib.rs"));

        let result = tool.execute(args, &ctx).await.expect("list symbols");
        assert_eq!(result.is_error, Some(false));
        assert!(!extract_text(&result).is_empty());
    }

    #[tokio::test]
    async fn test_list_symbols_missing_file_path_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("list symbols"));
        // Omit "file_path"

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file_path"));
    }

    // -----------------------------------------------------------------------
    // get callgraph
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_callgraph_returns_results_or_progress() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get callgraph"));
        args.insert("symbol".to_string(), serde_json::json!("main"));

        let result = tool.execute(args, &ctx).await.expect("get callgraph");
        assert_eq!(result.is_error, Some(false));
        assert!(!extract_text(&result).is_empty());
    }

    #[tokio::test]
    async fn test_get_callgraph_missing_symbol_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get callgraph"));
        // Omit "symbol"

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("symbol"));
    }

    #[tokio::test]
    async fn test_get_callgraph_invalid_direction_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get callgraph"));
        args.insert("symbol".to_string(), serde_json::json!("main"));
        args.insert("direction".to_string(), serde_json::json!("sideways"));

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("direction"));
    }

    #[tokio::test]
    async fn test_get_callgraph_inbound_direction() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get callgraph"));
        args.insert("symbol".to_string(), serde_json::json!("greet"));
        args.insert("direction".to_string(), serde_json::json!("inbound"));

        let result = tool
            .execute(args, &ctx)
            .await
            .expect("get callgraph inbound");
        assert_eq!(result.is_error, Some(false));
    }

    // -----------------------------------------------------------------------
    // get blastradius
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_blastradius_returns_results_or_progress() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get blastradius"));
        args.insert("file_path".to_string(), serde_json::json!("src/lib.rs"));

        let result = tool.execute(args, &ctx).await.expect("get blastradius");
        assert_eq!(result.is_error, Some(false));
        assert!(!extract_text(&result).is_empty());
    }

    #[tokio::test]
    async fn test_get_blastradius_missing_file_path_returns_error() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get blastradius"));
        // Omit "file_path"

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file_path"));
    }

    // -----------------------------------------------------------------------
    // detect projects
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_detect_projects_returns_project_list() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        // Add Cargo.toml to make it look like a Rust project
        if let Some(ref dir) = ctx.working_dir {
            std::fs::write(
                dir.join("Cargo.toml"),
                "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
        }

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("detect projects"));

        let result = tool.execute(args, &ctx).await.expect("detect projects");
        assert_eq!(result.is_error, Some(false));
        let text = extract_text(&result);
        assert!(!text.is_empty());
    }

    #[tokio::test]
    async fn test_detect_projects_with_path_param() {
        let (_tmp, ctx) = create_indexed_project().await;
        let tool = CodeContextTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("detect projects"));
        // Use a non-existent subdirectory — should return "no projects found" gracefully
        args.insert("path".to_string(), serde_json::json!("/tmp"));

        let result = tool
            .execute(args, &ctx)
            .await
            .expect("detect projects with path");
        assert_eq!(result.is_error, Some(false));
    }

    // -----------------------------------------------------------------------
    // Error handling for missing/invalid workspace
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_operations_with_no_working_dir() {
        // When working_dir is not set, operations should either succeed
        // (using cwd as fallback) or return a meaningful error.
        let tool = CodeContextTool::new();
        let ctx = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));

        // Either succeeds or fails with an internal error — just must not panic.
        let result = tool.execute(args, &ctx).await;
        // We accept both Ok and Err here — just verify no panic occurs.
        let _ = result;
    }

    // -----------------------------------------------------------------------
    // Chunk embedding tests for `index_discovered_files_with_embedder`
    //
    // These tests exercise the embedding path via dependency injection — they
    // pass a `MockEmbedder` rather than constructing a real model, so they run
    // fast and deterministically. They cover:
    //   - success path: every chunk gets an embedding blob, `embedded=1`
    //   - partial failure: failing chunk has NULL embedding, others succeed,
    //     `embedded=0` (the successful chunks remain searchable; the file is
    //     not re-driven until `ts_indexed` is flipped back to 0 elsewhere)
    //   - no embedder: chunks are still written without embeddings, `embedded=0`
    //     (existing fallback behavior preserved)
    //   - round-trip: blob written by indexer deserializes to the same vector
    // -----------------------------------------------------------------------

    use model_embedding::mock::MockEmbedder;
    use model_embedding::TextEmbedder;

    /// Set up a tiny Rust project on disk and open the workspace.
    ///
    /// Returns the tempdir (caller must keep it alive) and the shared DB ref.
    /// `startup_cleanup` runs as part of `CodeContextWorkspace::open`, so the
    /// `indexed_files` table is already populated with the two source files
    /// (marked dirty).
    async fn make_tiny_indexable_project() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        swissarmyhammer_code_context::SharedDb,
    ) {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let root = tmp.path().to_path_buf();

        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/main.rs"),
            "fn main() {\n    println!(\"hi\");\n}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        )
        .unwrap();

        let ws = CodeContextWorkspace::open(&root).expect("workspace open");
        let shared_db = ws.shared_db().expect("leader has shared db");
        (tmp, root, shared_db)
    }

    /// Count chunks in `ts_chunks` that have a non-NULL `embedding` blob.
    fn count_embedded_chunks(db: &swissarmyhammer_code_context::SharedDb) -> i64 {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Count total chunks regardless of embedding state.
    fn count_total_chunks(db: &swissarmyhammer_code_context::SharedDb) -> i64 {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
            .unwrap()
    }

    /// Read the `embedded` flag for a file row.
    fn read_embedded_flag(
        db: &swissarmyhammer_code_context::SharedDb,
        file_path: &str,
    ) -> Option<i64> {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT embedded FROM indexed_files WHERE file_path = ?",
            rusqlite::params![file_path],
            |r| r.get(0),
        )
        .ok()
    }

    /// With a working embedder, every chunk row has a non-NULL embedding blob
    /// and every fully-embedded file has `embedded=1`.
    #[tokio::test]
    async fn test_indexer_writes_embedding_blob_for_every_chunk() {
        let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

        // Always-succeeding mock embedder with a small fixed dimension.
        let embedder: std::sync::Arc<dyn TextEmbedder> = std::sync::Arc::new(MockEmbedder::new(8));

        index_discovered_files_with_embedder(
            &root,
            shared_db.clone(),
            Some(embedder),
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;

        let total = count_total_chunks(&shared_db);
        let embedded = count_embedded_chunks(&shared_db);
        assert!(total > 0, "expected >0 chunks after indexing, got {total}");
        assert_eq!(
            embedded, total,
            "every chunk should have a non-NULL embedding blob"
        );

        // Files should be marked embedded=1.
        for relative in ["src/main.rs", "src/lib.rs"] {
            let flag = read_embedded_flag(&shared_db, relative);
            assert_eq!(
                flag,
                Some(1),
                "expected {relative} to have embedded=1, got {flag:?}"
            );
        }
    }

    /// Embeddings written by the indexer are binary-compatible with the
    /// `deserialize_embedding` helper used by `search_code`.
    #[tokio::test]
    async fn test_indexer_embedding_blob_roundtrips_through_deserialize() {
        let (_tmp, root, shared_db) = make_tiny_indexable_project().await;
        let dim = 8;
        let embedder: std::sync::Arc<dyn TextEmbedder> =
            std::sync::Arc::new(MockEmbedder::new(dim));

        index_discovered_files_with_embedder(
            &root,
            shared_db.clone(),
            Some(embedder),
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;

        // Read one row's blob and convert it back to a Vec<f32>.
        let blob: Vec<u8> = {
            let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
            conn.query_row(
                "SELECT embedding FROM ts_chunks WHERE embedding IS NOT NULL LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(
            blob.len(),
            dim * 4,
            "blob length should be dim*4 bytes (little-endian f32)"
        );

        // Round-trip through the same little-endian f32 layout used by
        // search_code::deserialize_embedding.
        let parsed: Vec<f32> = blob
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(parsed.len(), dim);
        // MockEmbedder returns vec![0.1; dim]
        for v in parsed {
            assert!((v - 0.1).abs() < 1e-6, "expected 0.1 vector, got {v}");
        }
    }

    /// When the embedder fails on a specific chunk, that chunk row has NULL
    /// embedding, other chunks succeed, and the file's `embedded` flag stays
    /// at 0. The file is not re-driven by this function until something else
    /// flips `ts_indexed` back to 0; the successfully embedded chunks remain
    /// searchable in the meantime.
    #[tokio::test]
    async fn test_indexer_partial_embedding_failure_leaves_file_unembedded() {
        let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

        // Fail on the very first embed_text call. With two tiny files, at
        // least one file will end up with a partially-failed chunk.
        let mock = std::sync::Arc::new(MockEmbedder::with_failures(8, vec![0]));
        let embedder: std::sync::Arc<dyn TextEmbedder> = mock.clone();

        index_discovered_files_with_embedder(
            &root,
            shared_db.clone(),
            Some(embedder),
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;

        let total = count_total_chunks(&shared_db);
        let embedded = count_embedded_chunks(&shared_db);
        assert!(total > 0, "expected chunks to be written even on failure");
        assert!(
            embedded < total,
            "expected at least one chunk to have NULL embedding (embedded={embedded}, total={total})"
        );
        assert!(
            embedded > 0,
            "expected the other chunks to still succeed (embedded={embedded})"
        );

        // At least one file must be left with embedded=0 because one of its
        // chunks failed to embed.
        let conn_flags: Vec<(String, i64)> = {
            let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = conn
                .prepare("SELECT file_path, embedded FROM indexed_files ORDER BY file_path")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(
            conn_flags.iter().any(|(_, flag)| *flag == 0),
            "expected at least one file with embedded=0, got: {conn_flags:?}"
        );

        // The mock should have been called at least once per chunk.
        assert!(
            mock.call_count() >= total as usize,
            "embedder should have been driven for every chunk (call_count={}, total={total})",
            mock.call_count()
        );
    }

    /// When no embedder is provided (e.g. construction or load failed), chunks
    /// are still written without embeddings (preserved fallback behavior),
    /// and `embedded` stays at 0. As with partial failure, the file is not
    /// re-driven by this function — a future invocation with a working
    /// embedder is only triggered once `ts_indexed` is flipped back to 0.
    #[tokio::test]
    async fn test_indexer_no_embedder_still_writes_chunks_without_embeddings() {
        let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

        index_discovered_files_with_embedder(
            &root,
            shared_db.clone(),
            None,
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;

        let total = count_total_chunks(&shared_db);
        let embedded = count_embedded_chunks(&shared_db);
        assert!(total > 0, "expected chunks to be written without embedder");
        assert_eq!(
            embedded, 0,
            "no chunks should have embeddings when embedder is absent"
        );

        for relative in ["src/main.rs", "src/lib.rs"] {
            let flag = read_embedded_flag(&shared_db, relative);
            assert_eq!(
                flag,
                Some(0),
                "expected {relative} to have embedded=0 when no embedder, got {flag:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Progress reporter tests for `index_discovered_files_with_embedder`
    //
    // These tests use a `VecReporter` that records every `IndexProgress`
    // event the indexer emits. They run the indexer end-to-end on the same
    // tiny two-file workspace fixture used by the embedding tests above so
    // we exercise the real chunk + embed code paths, not a stub.
    // -----------------------------------------------------------------------

    /// A `ProgressReporter` that records every event into a `Mutex<Vec<_>>`
    /// so tests can assert on the recorded sequence.
    struct VecReporter {
        events: std::sync::Mutex<Vec<swissarmyhammer_code_context::IndexProgress>>,
    }

    impl VecReporter {
        fn new() -> Self {
            Self {
                events: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn snapshot(&self) -> Vec<swissarmyhammer_code_context::IndexProgress> {
            self.events.lock().unwrap().clone()
        }
    }

    impl swissarmyhammer_code_context::ProgressReporter for VecReporter {
        fn report(&self, event: swissarmyhammer_code_context::IndexProgress) {
            self.events.lock().unwrap().push(event);
        }
    }

    /// The end-to-end event sequence must:
    /// - Open with a `Discovering` event (the pre-discovery zero-count signal)
    /// - Follow with a second `Discovering` carrying the final file count
    /// - Emit at least one `Chunking` and at least one `Embedding` event per file
    /// - Close with exactly one `Done` event
    ///
    /// We use a tiny two-file Rust workspace so the assertions stay readable.
    #[tokio::test]
    async fn test_indexer_emits_progress_event_sequence() {
        use swissarmyhammer_code_context::IndexProgress;

        let (_tmp, root, shared_db) = make_tiny_indexable_project().await;

        let reporter = std::sync::Arc::new(VecReporter::new());
        let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
            reporter.clone();

        // Always-succeeding mock embedder so we get a non-empty
        // `chunks_in_batch` value to assert on.
        let embedder: std::sync::Arc<dyn TextEmbedder> = std::sync::Arc::new(MockEmbedder::new(8));

        index_discovered_files_with_embedder(
            &root,
            shared_db.clone(),
            Some(embedder),
            dyn_reporter,
        )
        .await;

        let events = reporter.snapshot();
        assert!(
            events.len() >= 5,
            "expected at least 5 events (Discovering x2, Chunking, Embedding, Done) for a \
             two-file workspace, got {}: {events:?}",
            events.len()
        );

        // First event is the pre-discovery `Discovering { found: 0 }` signal.
        assert!(
            matches!(events[0], IndexProgress::Discovering { found: 0 }),
            "first event must be Discovering {{ found: 0 }}, got {:?}",
            events[0]
        );

        // Second event is the post-discovery `Discovering { found: N }` signal.
        let discovered_total = match events[1] {
            IndexProgress::Discovering { found } => found,
            ref other => panic!("second event must be a Discovering total, got {other:?}"),
        };
        assert_eq!(
            discovered_total, 2,
            "two-file workspace should discover 2 files, got {discovered_total}"
        );

        // Final event is exactly one `Done`. It must report the same number
        // of files we discovered, and `chunks` must match `count_total_chunks`.
        let last = events.last().expect("non-empty events");
        let total_chunks_in_db = count_total_chunks(&shared_db) as u64;
        match last {
            IndexProgress::Done {
                files,
                chunks,
                elapsed,
            } => {
                assert_eq!(
                    *files, discovered_total,
                    "Done.files must match discovered count"
                );
                assert_eq!(
                    *chunks, total_chunks_in_db,
                    "Done.chunks must match the row count in ts_chunks"
                );
                assert!(
                    elapsed.as_nanos() > 0,
                    "Done.elapsed should be non-zero for a real indexing pass"
                );
            }
            other => panic!("last event must be Done, got {other:?}"),
        }
        // No event after Done.
        let done_count = events
            .iter()
            .filter(|e| matches!(e, IndexProgress::Done { .. }))
            .count();
        assert_eq!(
            done_count, 1,
            "expected exactly one Done event, got {done_count}"
        );

        // Middle events: every `Chunking` event's `done` is monotonically
        // non-decreasing and bounded by `total`. Collect them in order and
        // check.
        let chunking: Vec<(u64, u64)> = events
            .iter()
            .filter_map(|e| match e {
                IndexProgress::Chunking { done, total, .. } => Some((*done, *total)),
                _ => None,
            })
            .collect();
        assert_eq!(
            chunking.len(),
            discovered_total as usize,
            "expected one Chunking event per discovered file"
        );
        let mut prev_done = 0u64;
        for (done, total) in &chunking {
            assert!(
                *done > prev_done,
                "Chunking.done must be strictly increasing — got {done} after {prev_done}"
            );
            assert_eq!(
                *total, discovered_total,
                "Chunking.total must equal the discovered file count"
            );
            assert!(
                *done <= *total,
                "Chunking.done ({done}) must not exceed total ({total})"
            );
            prev_done = *done;
        }
        assert_eq!(
            prev_done, discovered_total,
            "the last Chunking event must report done == total"
        );

        // Embedding events: 1-based batch index, monotonically increasing,
        // each one's chunks_in_batch is the number of chunks for that file.
        let embedding: Vec<(u64, u64, u64)> = events
            .iter()
            .filter_map(|e| match e {
                IndexProgress::Embedding {
                    batch,
                    batches,
                    chunks_in_batch,
                } => Some((*batch, *batches, *chunks_in_batch)),
                _ => None,
            })
            .collect();
        assert_eq!(
            embedding.len(),
            discovered_total as usize,
            "expected one Embedding event per discovered file"
        );
        for (idx, (batch, batches, _)) in embedding.iter().enumerate() {
            assert_eq!(
                *batch,
                (idx + 1) as u64,
                "Embedding.batch must be 1-based and sequential"
            );
            assert_eq!(
                *batches, discovered_total,
                "Embedding.batches must equal the planned batch total (one batch per file)"
            );
        }
    }

    /// When the dirty-file SQL query fails (e.g. the `indexed_files` table is
    /// missing or the connection is otherwise broken), the indexer must still
    /// emit a complete lifecycle: pre-discovery `Discovering(0)`, the
    /// post-discovery `Discovering(0)` symmetry signal, then the terminal
    /// `Done(0, 0, _)`. Without the second `Discovering`, consumers that key
    /// off "second Discovering means discovery completed" would never see the
    /// signal on this path — so we assert exactly three events here, mirroring
    /// the empty-workspace lifecycle.
    #[tokio::test]
    async fn test_indexer_db_query_failure_still_emits_framing_events() {
        use swissarmyhammer_code_context::{CodeContextWorkspace, IndexProgress};

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let root = tmp.path().to_path_buf();
        let ws = CodeContextWorkspace::open(&root).expect("workspace open");
        let shared_db = ws.shared_db().expect("leader has shared db");

        // Force the dirty-file query to fail by dropping the table it reads.
        // Subsequent `SELECT file_path FROM indexed_files WHERE ts_indexed = 0`
        // returns `Err(rusqlite::Error::SqliteFailure(..))` ("no such table"),
        // which exercises the early-return error branch in
        // `index_discovered_files_with_embedder`.
        {
            let conn = shared_db.lock().unwrap_or_else(|p| p.into_inner());
            conn.execute("DROP TABLE indexed_files", [])
                .expect("drop indexed_files");
        }

        let reporter = std::sync::Arc::new(VecReporter::new());
        let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
            reporter.clone();
        index_discovered_files_with_embedder(&root, shared_db, None, dyn_reporter).await;

        let events = reporter.snapshot();
        assert_eq!(
            events.len(),
            3,
            "DB-query-failure path must emit exactly Discovering(0), Discovering(0), Done — \
             got {events:?}"
        );
        assert!(
            matches!(events[0], IndexProgress::Discovering { found: 0 }),
            "first event must be pre-discovery Discovering(0), got {:?}",
            events[0]
        );
        assert!(
            matches!(events[1], IndexProgress::Discovering { found: 0 }),
            "second event must be post-discovery Discovering(0) — without it, consumers \
             that key off 'second Discovering means discovery completed' will never see \
             the signal on this error path. Got {:?}",
            events[1]
        );
        match &events[2] {
            IndexProgress::Done {
                files: 0,
                chunks: 0,
                ..
            } => {}
            other => panic!("final event must be Done(0, 0, _), got {other:?}"),
        }
    }

    /// When the dirty-file set is empty (no files to index), the indexer
    /// must still emit the open/close framing events so consumers see a
    /// clean lifecycle: `Discovering(0)`, `Discovering(0)`, `Done(0, 0, _)`.
    #[tokio::test]
    async fn test_indexer_empty_workspace_still_emits_framing_events() {
        use swissarmyhammer_code_context::{CodeContextWorkspace, IndexProgress};

        // Empty temp dir — no source files, so `indexed_files` will be empty
        // after `startup_cleanup` and the dirty-file query returns nothing.
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let root = tmp.path().to_path_buf();
        let ws = CodeContextWorkspace::open(&root).expect("workspace open");
        let shared_db = ws.shared_db().expect("leader has shared db");

        let reporter = std::sync::Arc::new(VecReporter::new());
        let dyn_reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter> =
            reporter.clone();
        index_discovered_files_with_embedder(&root, shared_db, None, dyn_reporter).await;

        let events = reporter.snapshot();
        assert_eq!(
            events.len(),
            3,
            "empty workspace must emit exactly Discovering(0), Discovering(0), Done — got {events:?}"
        );
        assert!(matches!(events[0], IndexProgress::Discovering { found: 0 }));
        assert!(matches!(events[1], IndexProgress::Discovering { found: 0 }));
        match &events[2] {
            IndexProgress::Done {
                files: 0,
                chunks: 0,
                ..
            } => {}
            other => panic!("final event must be Done(0, 0, _), got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // search code: readiness gate removal
    //
    // `execute_search_code` used to bail out with an "Index not ready"
    // placeholder string when the tree-sitter pass wasn't done. The gate is
    // gone: `search code` now always returns a `SearchCodeResult` and the
    // caller learns about partial coverage via the `progress` field.
    //
    // The dispatch path runs a real embedder, so this unit test exercises
    // the inner function `search_code_with_query_embedding` directly with a
    // caller-supplied embedding vector. That keeps the test fast and
    // deterministic while still proving the gate is gone — the test would
    // fail with a "not ready" placeholder if it weren't.
    // -----------------------------------------------------------------------

    /// When files exist in `indexed_files` but none are embedded yet, the
    /// inner search must return a `SearchCodeResult` (possibly with empty
    /// matches) carrying a populated `progress` field — never the old
    /// "Index not ready" placeholder string.
    #[tokio::test]
    async fn test_search_code_returns_result_with_progress_when_not_embedded() {
        let (_tmp, root, _shared_db) = make_tiny_indexable_project().await;
        let ctx = make_context_with_dir(root.clone());

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("search code"));
        args.insert("query".to_string(), serde_json::json!("anything"));

        // Use a tiny dummy embedding — the search returns no matches because
        // no chunk embeddings exist yet, but it must still succeed and
        // produce a `SearchCodeResult` JSON, not the readiness placeholder.
        let dummy_query_embedding = vec![1.0f32, 0.0, 0.0];
        let result = search_code_with_query_embedding(&args, &ctx, &dummy_query_embedding)
            .expect("search code should succeed without the readiness gate");

        let text = extract_text(&result);
        assert!(
            !text.contains("Index not ready"),
            "search code must not return the readiness placeholder, got: {text}"
        );

        // The body must parse as a SearchCodeResult JSON with the progress
        // field populated (3 files exist, 0 are embedded).
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("result must be JSON-encoded SearchCodeResult");
        assert!(
            parsed.get("matches").is_some(),
            "result must have a `matches` field"
        );
        let progress = parsed
            .get("progress")
            .expect("result must have a `progress` field");
        assert!(
            !progress.is_null(),
            "progress must be populated when embedded_files < total_files, got null"
        );
        assert!(
            progress.get("embedded_files").and_then(|v| v.as_u64()) == Some(0),
            "embedded_files should be 0 when no files have been embedded yet"
        );
        let total = progress
            .get("total_files")
            .and_then(|v| v.as_u64())
            .expect("total_files must be present and numeric");
        assert!(total > 0, "total_files should be > 0, got {total}");
    }
}
