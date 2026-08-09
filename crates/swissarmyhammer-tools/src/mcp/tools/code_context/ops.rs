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

/// Verb of every operation that reads something out of the index.
const VERB_GET: &str = "get";
/// Verb of every operation that matches a query against the index.
const VERB_SEARCH: &str = "search";
/// Verb of the operation that enumerates a file's contents.
const VERB_LIST: &str = "list";
/// Verb of the operation that matches a regular expression.
const VERB_GREP: &str = "grep";
/// Verb of the operation that resets the index.
const VERB_REBUILD: &str = "rebuild";
/// Verb of the operation that wipes the index.
const VERB_CLEAR: &str = "clear";
/// Verb of the operation that reports language server health.
const VERB_LSP: &str = "lsp";
/// Verb of every operation that reports what it detects in the source.
const VERB_FIND: &str = "find";
/// Verb of the operation that runs a tree-sitter query.
const VERB_QUERY: &str = "query";
/// Verb of the operation that identifies project types.
const VERB_DETECT: &str = "detect";

/// Noun of the operations that act on a single symbol.
const NOUN_SYMBOL: &str = "symbol";
/// Noun of the operation that acts on all symbols of a file.
const NOUN_SYMBOLS: &str = "symbols";
/// Noun of the operations that act on stored code chunks.
const NOUN_CODE: &str = "code";
/// Noun of the call graph traversal operation.
const NOUN_CALLGRAPH: &str = "callgraph";
/// Noun of the callers-of-a-position operation.
const NOUN_INBOUND_CALLS: &str = "inbound_calls";
/// Noun of the workspace-wide symbol search operation.
const NOUN_WORKSPACE_SYMBOL: &str = "workspace_symbol";
/// Noun of the change-impact operation.
const NOUN_BLASTRADIUS: &str = "blastradius";
/// Noun of every operation that reports or resets index health.
const NOUN_STATUS: &str = "status";
/// Noun of the re-indexing operation.
const NOUN_INDEX: &str = "index";
/// Noun of the duplicated-code operation.
const NOUN_DUPLICATES: &str = "duplicates";
/// Noun of the token-identical-block operation.
const NOUN_DUPLICATION: &str = "duplication";
/// Noun of the tree-sitter query operation.
const NOUN_AST: &str = "ast";
/// Noun of the commented-out-code operation.
const NOUN_COMMENTED_CODE: &str = "commented_code";
/// Noun of the project-type detection operation.
const NOUN_PROJECTS: &str = "projects";
/// Noun of the rename-preview operation.
const NOUN_RENAME_EDITS: &str = "rename_edits";
/// Noun of the errors-and-warnings operation.
const NOUN_DIAGNOSTICS: &str = "diagnostics";
/// Noun of the go-to-definition operation.
const NOUN_DEFINITION: &str = "definition";
/// Noun of the go-to-type-definition operation.
const NOUN_TYPE_DEFINITION: &str = "type_definition";
/// Noun of the hover information operation.
const NOUN_HOVER: &str = "hover";
/// Noun of the find-all-references operation.
const NOUN_REFERENCES: &str = "references";
/// Noun of the find-implementations operation.
const NOUN_IMPLEMENTATIONS: &str = "implementations";
/// Noun of the quickfix-and-refactor operation.
const NOUN_CODE_ACTIONS: &str = "code_actions";

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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_SYMBOL
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
        VERB_SEARCH
    }
    fn noun(&self) -> &'static str {
        NOUN_SYMBOL
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
        VERB_LIST
    }
    fn noun(&self) -> &'static str {
        NOUN_SYMBOLS
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
        VERB_GREP
    }
    fn noun(&self) -> &'static str {
        NOUN_CODE
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_CALLGRAPH
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_INBOUND_CALLS
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
        VERB_SEARCH
    }
    fn noun(&self) -> &'static str {
        NOUN_WORKSPACE_SYMBOL
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_BLASTRADIUS
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_STATUS
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
        VERB_REBUILD
    }
    fn noun(&self) -> &'static str {
        NOUN_INDEX
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
        VERB_CLEAR
    }
    fn noun(&self) -> &'static str {
        NOUN_STATUS
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
        VERB_LSP
    }
    fn noun(&self) -> &'static str {
        NOUN_STATUS
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
        VERB_SEARCH
    }
    fn noun(&self) -> &'static str {
        NOUN_CODE
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
        VERB_FIND
    }
    fn noun(&self) -> &'static str {
        NOUN_DUPLICATES
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
        VERB_QUERY
    }
    fn noun(&self) -> &'static str {
        NOUN_AST
    }
    fn description(&self) -> &'static str {
        "Execute tree-sitter S-expression queries against parsed ASTs for structural code search"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        QUERY_AST_PARAMS
    }
}

/// Operation metadata for the token-identical duplicate gate.
#[derive(Debug, Default)]
pub struct FindDuplication;

static FIND_DUPLICATION_PARAMS: &[ParamMeta] = &[ParamMeta::new("files")
    .description("File paths to read, relative to the workspace root or absolute")
    .param_type(ParamType::Array)];

impl Operation for FindDuplication {
    fn verb(&self) -> &'static str {
        VERB_FIND
    }
    fn noun(&self) -> &'static str {
        NOUN_DUPLICATION
    }
    fn description(&self) -> &'static str {
        "Report each pair of token-identical blocks the files repeat, in one file or across two, one `path:line: message` line per pair"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        FIND_DUPLICATION_PARAMS
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
        VERB_FIND
    }
    fn noun(&self) -> &'static str {
        NOUN_COMMENTED_CODE
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
        VERB_DETECT
    }
    fn noun(&self) -> &'static str {
        NOUN_PROJECTS
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_RENAME_EDITS
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_DIAGNOSTICS
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_DEFINITION
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_TYPE_DEFINITION
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_HOVER
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_REFERENCES
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_IMPLEMENTATIONS
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
        VERB_GET
    }
    fn noun(&self) -> &'static str {
        NOUN_CODE_ACTIONS
    }
    fn description(&self) -> &'static str {
        "Get code actions (quickfixes, refactors) for a range (live LSP only). Returns empty when no LSP is available."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_CODE_ACTIONS_PARAMS
    }
}

/// Declares the roster of `code_context` operations.
///
/// One invocation names each operation type once. The macro builds that type's
/// process-wide singleton and its entry in `CODE_CONTEXT_OPERATIONS`, in the
/// order given, so the singletons and the roster cannot drift apart.
macro_rules! code_context_roster {
    ($($op:ty),+ $(,)?) => {
        /// Every operation instance the tool dispatches, in schema order.
        static CODE_CONTEXT_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
            vec![$({
                static OP: Lazy<$op> = Lazy::new(<$op as Default>::default);
                &*OP as &dyn Operation
            }),+]
        });
    };
}

code_context_roster![
    GetSymbol,
    SearchSymbol,
    ListSymbols,
    GrepCode,
    SearchCode,
    FindDuplicates,
    QueryAst,
    FindDuplication,
    FindCommentedCode,
    GetCallgraph,
    GetBlastradius,
    GetCodeStatus,
    RebuildIndex,
    ClearStatus,
    LspStatus,
    DetectProjects,
    GetRenameEdits,
    GetDiagnostics,
    GetInboundCalls,
    WorkspaceSymbolLive,
    GetDefinition,
    GetTypeDefinition,
    GetHover,
    GetReferences,
    GetImplementations,
    GetCodeActions,
];

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
