//! One metadata struct per `code_context` operation, and the roster the tool's
//! schema and dispatch are generated from.
//!
//! An [`Operation`] here declares an op's verb, noun, description and
//! parameters — never its behaviour. The handler that answers it lives in
//! [`execute`](super::execute), [`indexing`](super::indexing),
//! [`status`](super::status), [`lsp_ops`](super::lsp_ops) or
//! [`detect`](super::detect).

use once_cell::sync::Lazy;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

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

/// Operation metadata for the commented-out-code re-parse verdict.
#[derive(Debug, Default)]
pub struct FindCommentedCode;

static FIND_COMMENTED_CODE_PARAMS: &[ParamMeta] = &[ParamMeta::new("files")
    .description("File paths to read, relative to the workspace root or absolute")
    .param_type(ParamType::Array)];

impl Operation for FindCommentedCode {
    fn verb(&self) -> &'static str {
        "find"
    }
    fn noun(&self) -> &'static str {
        "commented_code"
    }
    fn description(&self) -> &'static str {
        "Report each comment block that re-parses as code in the file's own language, one `path:line: message` line per block"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        FIND_COMMENTED_CODE_PARAMS
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
static FIND_COMMENTED_CODE_OP: Lazy<FindCommentedCode> = Lazy::new(FindCommentedCode::default);
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
        &*FIND_COMMENTED_CODE_OP as &dyn Operation,
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

/// The operations the `code_context` tool dispatches, in the order the schema
/// lists them.
///
/// The ONE roster. The tool's [`operations`](crate::mcp::tool_registry::McpTool::operations),
/// both schema surfaces and the schema tests all read it, so an op joins every
/// one of them the moment it joins this list — there is no second list to keep
/// in step.
pub fn code_context_operations() -> &'static [&'static dyn Operation] {
    CODE_CONTEXT_OPERATIONS.as_slice()
}
