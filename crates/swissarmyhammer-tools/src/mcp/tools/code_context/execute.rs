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

use swissarmyhammer_code_context::{
    find_commented_code, find_duplication, BlastRadiusOptions, CallGraphDirection,
    CallGraphOptions, FindDuplicatesOptions, GetSymbolOptions, GrepOptions, QueryAstOptions,
    SearchCodeOptions, SearchSymbolOptions,
};

use super::support::{
    check_ts_readiness, context_err, extract_f32_param, extract_optional_str,
    extract_optional_string, extract_optional_string_array, extract_optional_usize,
    extract_required_str, extract_required_str_array, extract_u32_param, extract_usize_param,
    open_workspace, resolve_working_dir, resolve_workspace_root, DEFAULT_MAX_RESULTS,
};

/// Default hit count for `search code` when the caller omits `top_k`.
const DEFAULT_TOP_K: usize = 10;

/// Default cosine-similarity floor for `find duplicates`.
///
/// A pair of chunks below this score is not reported as a duplicate.
const DEFAULT_MIN_SIMILARITY: f32 = 0.85;

/// Default smallest chunk, in bytes, that `find duplicates` compares.
///
/// Chunks under this size are too short to make a meaningful duplicate.
const DEFAULT_MIN_CHUNK_BYTES: usize = 100;

/// Default cap on how many matches `find duplicates` reports for one chunk.
const DEFAULT_MAX_PER_CHUNK: usize = 5;

/// Default traversal depth for `get callgraph`.
const DEFAULT_CALLGRAPH_MAX_DEPTH: u32 = 2;

/// Default traversal depth for `get blastradius`.
const DEFAULT_BLAST_RADIUS_MAX_HOPS: u32 = 3;

/// Execute the "get symbol" operation.
///
/// Retrieves symbol source text using multi-tier fuzzy matching
/// (exact, suffix, case-insensitive, fuzzy).
pub(super) fn execute_get_symbol(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let query = extract_required_str(args, "query")?;

    let options = GetSymbolOptions {
        max_results: extract_optional_usize(args, "max_results"),
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
    let query = extract_required_str(args, "query")?;

    let options = SearchSymbolOptions {
        kind: extract_optional_string(args, "kind"),
        max_results: extract_optional_usize(args, "max_results"),
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
    let file_path = extract_required_str(args, "file_path")?;

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
    let pattern = extract_required_str(args, "pattern")?;

    let options = GrepOptions {
        language: extract_optional_string_array(args, "language"),
        files: extract_optional_string_array(args, "files"),
        max_results: extract_optional_usize(args, "max_results"),
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
    let query = extract_required_str(args, "query")?;

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
    let top_k = extract_usize_param(args, "top_k", DEFAULT_TOP_K);

    // Hybrid fusion uses the internal default signal weights and no fused-score
    // floor; the MCP surface exposes only `query`, `top_k`, `language`, and
    // `file_pattern` (the `min_similarity` knob is gone for `search code`).
    let options = SearchCodeOptions {
        top_k,
        language: extract_optional_string_array(args, "language"),
        file_pattern: extract_optional_string(args, "file_pattern"),
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
    let file_path = extract_required_str(args, "file_path")?;

    let options = FindDuplicatesOptions {
        min_similarity: extract_f32_param(args, "min_similarity", DEFAULT_MIN_SIMILARITY),
        min_chunk_bytes: extract_usize_param(args, "min_chunk_bytes", DEFAULT_MIN_CHUNK_BYTES),
        max_per_chunk: extract_usize_param(args, "max_per_chunk", DEFAULT_MAX_PER_CHUNK),
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
    let query_str = extract_required_str(args, "query")?;
    let language_name = extract_required_str(args, "language")?;

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

    let workspace_root = resolve_workspace_root(context);

    // Get file paths: either from explicit list or by scanning DB for files with matching extensions
    let file_paths: Vec<String> = if let Some(files) = extract_optional_string_array(args, "files")
    {
        files
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

    let options = QueryAstOptions {
        max_results: extract_usize_param(args, "max_results", DEFAULT_MAX_RESULTS),
    };

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

/// Execute the "find duplication" operation.
///
/// Reports every pair of token-identical blocks the named files repeat, as
/// PLAIN TEXT, one `path:line: message` line per pair. The shape matches
/// [`execute_find_commented_code`] and for the same reason: the
/// `duplication-parsed` tool rule runs this op through `sah tool` and the
/// review engine parses its stdout directly, where a JSON result would arrive
/// as YAML.
///
/// No workspace is opened. A clone pair is a fact about the files named, so
/// the op answers without the code-context index and runs in a scratch
/// directory that holds no `.code-context` database — which is where the
/// rule's doctor fixtures live.
pub(super) fn execute_find_duplication(
    args: &serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let files: Vec<&str> = extract_required_str_array(args, "files")?;

    let working_dir = resolve_working_dir(context);

    let report = find_duplication(&working_dir, &files)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join("\n");
    Ok(CallToolResult::success(vec![Content::text(report)]))
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
    let files: Vec<&str> = extract_required_str_array(args, "files")?;

    let working_dir = resolve_working_dir(context);

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
    let symbol = extract_required_str(args, "symbol")?;

    let direction = match extract_optional_str(args, "direction") {
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

    let options = CallGraphOptions {
        symbol: symbol.to_string(),
        direction,
        max_depth: extract_u32_param(args, "max_depth", DEFAULT_CALLGRAPH_MAX_DEPTH),
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
    let file_path = extract_required_str(args, "file_path")?;

    let options = BlastRadiusOptions {
        file_path: file_path.to_string(),
        symbol: extract_optional_string(args, "symbol"),
        max_hops: extract_u32_param(args, "max_hops", DEFAULT_BLAST_RADIUS_MAX_HOPS),
    };

    let ws = open_workspace(context)?;
    if let Some(progress) = check_ts_readiness(&ws)? {
        return Ok(progress);
    }
    let result =
        swissarmyhammer_code_context::get_blastradius(&ws.db(), &options).map_err(context_err)?;
    json_result(&result)
}
