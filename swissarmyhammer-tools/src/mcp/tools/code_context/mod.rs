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
//! - `build status`: Mark files for re-indexing
//! - `clear status`: Wipe all index data
//!
//! Uses the `swissarmyhammer-code-context` crate for all operations,
//! opening a `CodeContextWorkspace` from the `ToolContext` working directory.

pub mod schema;
pub mod doctor;

use crate::mcp::tool_registry::{McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::{Annotated, CallToolResult, RawContent, RawTextContent};
use rmcp::ErrorData as McpError;
use std::path::Path;
use swissarmyhammer_code_context::{
    BlastRadiusOptions, BuildLayer, CallGraphDirection, CallGraphOptions, CodeContextWorkspace,
    GetSymbolOptions, GrepOptions, SearchSymbolOptions,
};
use swissarmyhammer_common::utils::find_git_repository_root_from;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_treesitter::IndexContext;

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
        .description("Filter by symbol kind: function, method, struct, class, interface, module, etc.")
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
pub struct BuildStatus;

static BUILD_STATUS_PARAMS: &[ParamMeta] = &[ParamMeta::new("layer")
    .description("Which indexing layer to reset: treesitter, lsp, or both (default: both)")
    .param_type(ParamType::String)];

impl Operation for BuildStatus {
    fn verb(&self) -> &'static str {
        "build"
    }
    fn noun(&self) -> &'static str {
        "status"
    }
    fn description(&self) -> &'static str {
        "Mark files for re-indexing by resetting indexed flags"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        BUILD_STATUS_PARAMS
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

// Static operation instances for schema generation
static GET_SYMBOL_OP: Lazy<GetSymbol> = Lazy::new(GetSymbol::default);
static SEARCH_SYMBOL_OP: Lazy<SearchSymbol> = Lazy::new(SearchSymbol::default);
static LIST_SYMBOLS_OP: Lazy<ListSymbols> = Lazy::new(ListSymbols::default);
static GREP_CODE_OP: Lazy<GrepCode> = Lazy::new(GrepCode::default);
static GET_CALLGRAPH_OP: Lazy<GetCallgraph> = Lazy::new(GetCallgraph::default);
static GET_BLASTRADIUS_OP: Lazy<GetBlastradius> = Lazy::new(GetBlastradius::default);
static GET_CODE_STATUS_OP: Lazy<GetCodeStatus> = Lazy::new(GetCodeStatus::default);
static BUILD_STATUS_OP: Lazy<BuildStatus> = Lazy::new(BuildStatus::default);
static CLEAR_STATUS_OP: Lazy<ClearStatus> = Lazy::new(ClearStatus::default);

static CODE_CONTEXT_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*GET_SYMBOL_OP as &dyn Operation,
        &*SEARCH_SYMBOL_OP as &dyn Operation,
        &*LIST_SYMBOLS_OP as &dyn Operation,
        &*GREP_CODE_OP as &dyn Operation,
        &*GET_CALLGRAPH_OP as &dyn Operation,
        &*GET_BLASTRADIUS_OP as &dyn Operation,
        &*GET_CODE_STATUS_OP as &dyn Operation,
        &*BUILD_STATUS_OP as &dyn Operation,
        &*CLEAR_STATUS_OP as &dyn Operation,
    ]
});

/// Unified code context tool providing symbol lookup, search, and graph operations.
#[derive(Default)]
pub struct CodeContextTool;

impl CodeContextTool {
    /// Creates a new CodeContextTool instance.
    pub fn new() -> Self {
        Self
    }
}

crate::impl_empty_doctorable!(CodeContextTool);

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

        match op_str {
            "get symbol" => execute_get_symbol(&arguments, context),
            "search symbol" => execute_search_symbol(&arguments, context),
            "list symbols" => execute_list_symbols(&arguments, context),
            "grep code" => execute_grep_code(&arguments, context),
            "get callgraph" => execute_get_callgraph(&arguments, context),
            "get blastradius" => execute_get_blastradius(&arguments, context),
            "get status" => execute_get_status(context),
            "build status" => execute_build_status(&arguments, context),
            "clear status" => execute_clear_status(context),
            "" => Err(McpError::invalid_params(
                "Missing 'op' field. Valid operations: 'get symbol', 'search symbol', 'list symbols', 'grep code', 'get callgraph', 'get blastradius', 'get status', 'build status', 'clear status'.",
                None,
            )),
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation '{}'. Valid operations: 'get symbol', 'search symbol', 'list symbols', 'grep code', 'get callgraph', 'get blastradius', 'get status', 'build status', 'clear status'",
                    other
                ),
                None,
            )),
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
    let workspace_root = find_git_repository_root_from(&working_dir)
        .unwrap_or(working_dir);

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

    Ok(CallToolResult {
        content: vec![Annotated::new(
            RawContent::Text(RawTextContent {
                text,
                meta: None,
            }),
            None,
        )],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    })
}

/// Convert a CodeContextError into an McpError.
fn context_err(e: swissarmyhammer_code_context::CodeContextError) -> McpError {
    McpError::internal_error(format!("{}", e), None)
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
        max_results: args.get("max_results").and_then(|v| v.as_u64()).map(|n| n as usize),
    };

    let ws = open_workspace(context)?;
    let result =
        swissarmyhammer_code_context::get_symbol(ws.db(), query, &options).map_err(context_err)?;
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
        max_results: args.get("max_results").and_then(|v| v.as_u64()).map(|n| n as usize),
    };

    let ws = open_workspace(context)?;
    let results = swissarmyhammer_code_context::search_symbol(ws.db(), query, &options)
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
        .ok_or_else(|| {
            McpError::invalid_params("Missing required parameter 'file_path'", None)
        })?;

    let ws = open_workspace(context)?;
    let results =
        swissarmyhammer_code_context::list_symbols(ws.db(), file_path).map_err(context_err)?;
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
        max_results: args.get("max_results").and_then(|v| v.as_u64()).map(|n| n as usize),
    };

    let ws = open_workspace(context)?;
    let result =
        swissarmyhammer_code_context::grep_code(ws.db(), pattern, &options).map_err(context_err)?;
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
    let result =
        swissarmyhammer_code_context::get_callgraph(ws.db(), &options).map_err(context_err)?;
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
        .ok_or_else(|| {
            McpError::invalid_params("Missing required parameter 'file_path'", None)
        })?;

    let symbol = args.get("symbol").and_then(|v| v.as_str()).map(String::from);
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
    let result =
        swissarmyhammer_code_context::get_blastradius(ws.db(), &options).map_err(context_err)?;
    json_result(&result)
}

/// Trigger tree-sitter indexing on discovered files.
///
/// This runs asynchronously after startup_cleanup discovers files.
/// Scans files, extracts chunks, and writes results to code-context DB.
async fn index_discovered_files_async(workspace_root: &Path) {
    // Open code-context workspace to access the database
    let ws = match CodeContextWorkspace::open(workspace_root) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("Failed to open code-context workspace for indexing: {}", e);
            return;
        }
    };

    // Create a tree-sitter index context for the workspace
    let mut ts_index = IndexContext::new(workspace_root.to_path_buf());

    // Run the scan to parse files and extract chunks
    match ts_index.scan().await {
        Ok(scan_result) => {
            tracing::debug!(
                "Tree-sitter indexing complete: parsed {} files in {}ms",
                scan_result.files_parsed,
                scan_result.total_time_ms
            );

            // Extract parsed files from tree-sitter index
            let files = ts_index.files();
            tracing::debug!("Extracted {} files from tree-sitter index", files.len());

            // TODO: For each parsed file, extract symbols and write to code-context DB:
            // 1. Get the ParsedFile for each path in files
            // 2. Extract symbols using ensure_ts_symbols()
            // 3. Generate call edges using generate_ts_call_edges()
            // 4. Write edges using write_ts_edges()
            // 5. Mark file as ts_indexed using appropriate DB update
            //
            // This requires:
            // - Opening each ParsedFile from ts_index
            // - Using code-context functions to write results
            // - Handling database transactions to avoid corruption
            // - Reporting progress back to get_status queries
        }
        Err(e) => {
            tracing::warn!("Tree-sitter indexing failed: {}", e);
        }
    }
}

/// Execute the "get status" operation.
///
/// Returns a health report with file counts, indexing progress, and chunk/edge counts.
/// Also includes LSP server availability from doctor check.
fn execute_get_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    let workspace_root = ws.workspace_root().to_path_buf();

    // Leader: trigger startup cleanup on first access to populate the index from disk
    if ws.is_leader() {
        let _ = swissarmyhammer_code_context::startup_cleanup(ws.db(), &workspace_root)
            .map_err(context_err);

        // After discovering files, spawn tree-sitter indexing in background
        // This runs asynchronously without blocking status response
        let workspace_root_clone = workspace_root.clone();
        tokio::spawn(async move {
            index_discovered_files_async(&workspace_root_clone).await;
        });
    }

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

    let result = swissarmyhammer_code_context::get_status(ws.db()).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "build status" operation.
///
/// Marks files for re-indexing by resetting the indexed flag for the specified layer.
fn execute_build_status(
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
    let result = swissarmyhammer_code_context::build_status(ws.db(), layer).map_err(context_err)?;
    json_result(&result)
}

/// Execute the "clear status" operation.
///
/// Wipes all index data from all tables and returns stats about what was cleared.
fn execute_clear_status(context: &ToolContext) -> Result<CallToolResult, McpError> {
    let ws = open_workspace(context)?;
    let result = swissarmyhammer_code_context::clear_status(ws.db()).map_err(context_err)?;
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
        assert_eq!(ops.len(), 9);
        assert!(ops.iter().any(|o| o.op_string() == "get symbol"));
        assert!(ops.iter().any(|o| o.op_string() == "search symbol"));
        assert!(ops.iter().any(|o| o.op_string() == "list symbols"));
        assert!(ops.iter().any(|o| o.op_string() == "grep code"));
        assert!(ops.iter().any(|o| o.op_string() == "get callgraph"));
        assert!(ops.iter().any(|o| o.op_string() == "get blastradius"));
        assert!(ops.iter().any(|o| o.op_string() == "get status"));
        assert!(ops.iter().any(|o| o.op_string() == "build status"));
        assert!(ops.iter().any(|o| o.op_string() == "clear status"));
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
        assert!(op_enum.contains(&serde_json::json!("get callgraph")));
        assert!(op_enum.contains(&serde_json::json!("get blastradius")));
        assert!(op_enum.contains(&serde_json::json!("get status")));
        assert!(op_enum.contains(&serde_json::json!("build status")));
        assert!(op_enum.contains(&serde_json::json!("clear status")));
    }

    #[test]
    fn test_code_context_tool_schema_has_operation_schemas() {
        let tool = CodeContextTool::new();
        let schema = tool.schema();

        let op_schemas = schema["x-operation-schemas"]
            .as_array()
            .expect("should have x-operation-schemas");
        assert_eq!(op_schemas.len(), 9);
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
}
