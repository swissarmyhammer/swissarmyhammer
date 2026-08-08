//! The `code_context` handlers backed by the stored tree-sitter index.
//!
//! Symbol lookup, symbol search, file listing, regex and semantic search,
//! duplicate detection, AST queries, the commented-code verdict, and the two
//! graph traversals. Each opens a workspace through
//! [`open_workspace`](super::support::open_workspace), gates on
//! [`check_ts_readiness`](super::support::check_ts_readiness) where a partial
//! index cannot answer, and renders its result as JSON.

use crate::mcp::op_tool_helpers::json_result;
use crate::mcp::tool_registry::ToolContext;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use swissarmyhammer_common::utils::find_git_repository_root_from;

use swissarmyhammer_code_context::{
    find_commented_code, BlastRadiusOptions, CallGraphDirection, CallGraphOptions,
    FindDuplicatesOptions, GetSymbolOptions, GrepOptions, QueryAstOptions, SearchCodeOptions,
    SearchSymbolOptions,
};

use super::support::{check_ts_readiness, context_err, open_workspace, DEFAULT_MAX_RESULTS};

/// Execute the "get symbol" operation.
///
/// Retrieves symbol source text using multi-tier fuzzy matching
/// (exact, suffix, case-insensitive, fuzzy).
pub(super) fn execute_get_symbol(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'query'", None))?;

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
pub(super) fn execute_search_symbol(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'query'", None))?;

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
pub(super) fn execute_list_symbols(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

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
pub(super) fn execute_grep_code(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'pattern'", None))?;

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
pub(super) async fn execute_search_code(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'query'", None))?;

    // Embed the query text
    use swissarmyhammer_embedding::{Embedder, TextEmbedder};
    let embedder = Embedder::default()
        .await
        .map_err(|e| McpError::internal_error(format!("failed to create embedder: {}", e), None))?;
    embedder.load().await.map_err(|e| {
        McpError::internal_error(format!("failed to load embedding model: {}", e), None)
    })?;
    let embed_result = embedder
        .embed_text(query)
        .await
        .map_err(|e| McpError::internal_error(format!("failed to embed query: {}", e), None))?;

    search_code_with_query_embedding(args, context, query, embed_result.embedding())
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
pub(super) fn search_code_with_query_embedding(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
    query_text: &str,
    query_embedding: &[f32],
) -> Result<CallToolResult, McpError> {
    let top_k = args
        .get("top_k")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(10);

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

    // Hybrid fusion uses the internal default signal weights and no fused-score
    // floor; the MCP surface exposes only `query`, `top_k`, `language`, and
    // `file_pattern` (the `min_similarity` knob is gone for `search code`).
    let options = SearchCodeOptions {
        top_k,
        language,
        file_pattern,
        ..Default::default()
    };

    let ws = open_workspace(context)?;
    let result =
        swissarmyhammer_code_context::search_code(&ws.db(), query_text, query_embedding, &options)
            .map_err(context_err)?;
    json_result(&result)
}

/// Execute the "find duplicates" operation.
///
/// For each chunk in the target file, finds similar chunks elsewhere —
/// in other files or elsewhere in the same file. A chunk never matches
/// itself.
pub(super) fn execute_find_duplicates(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

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
pub(super) fn execute_query_ast(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query_str = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'query'", None))?;

    let language_name = args
        .get("language")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'language'", None))?;

    // Resolve language via LanguageRegistry
    use swissarmyhammer_treesitter::LanguageRegistry;
    let registry = LanguageRegistry::global();
    let lang_config = registry
        .get_by_name(language_name)
        .ok_or_else(|| {
            McpError::invalid_params(
                format!("unsupported language '{}'. Use a language name like 'rust', 'python', 'typescript' and so on", language_name),
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
        .unwrap_or(DEFAULT_MAX_RESULTS);

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

/// Execute the "find commented_code" operation.
///
/// Reads each named file and reports the comment blocks that re-parse as code
/// in that file's own language. The result is PLAIN TEXT, one
/// `path:line: message` line per block and nothing else, because the
/// `no-commented-code-parsed` tool rule runs this op through `sah tool` and the
/// review engine parses its stdout directly. A JSON result would reach that
/// script as YAML, which the contract cannot read.
///
/// No workspace is opened. The verdict is a parse of the files named, so the
/// op answers without the code-context index and runs in a scratch directory
/// that holds no `.code-context` database — which is where the rule's doctor
/// fixtures live.
pub(super) fn execute_find_commented_code(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let files: Vec<&str> = args
        .get("files")
        .and_then(|value| value.as_array())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'files'", None))?
        .iter()
        .filter_map(|item| item.as_str())
        .collect();

    let working_dir = context
        .working_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    let report = find_commented_code(&working_dir, &files)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join("\n");
    Ok(CallToolResult::success(vec![Content::text(report)]))
}

/// Execute the "get callgraph" operation.
///
/// Traverses the call graph from a starting symbol in the specified direction.
pub(super) fn execute_get_callgraph(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'symbol'", None))?;

    let direction = match args.get("direction").and_then(|v| v.as_str()) {
        Some("inbound") => CallGraphDirection::Inbound,
        Some("outbound") | None => CallGraphDirection::Outbound,
        Some("both") => CallGraphDirection::Both,
        Some(other) => {
            return Err(McpError::invalid_params(
                format!(
                    "invalid direction '{}'. Valid values: 'inbound', 'outbound', 'both'",
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
pub(super) fn execute_get_blastradius(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required parameter 'file_path'", None))?;

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
