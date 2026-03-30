//! Search history operation for the shell tool.
//!
//! This module implements the "search history" operation which performs
//! semantic search across all command output using embeddings, finding
//! content by meaning rather than exact text match.

use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

use super::state::{self, ShellState};
use crate::mcp::tool_registry::BaseToolImpl;

/// Operation metadata for semantic search across command output history
#[derive(Debug, Default)]
pub struct SearchHistory;

static SEARCH_HISTORY_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("Natural language search query")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("command_id")
        .description("Filter to a specific command's output (optional)")
        .param_type(ParamType::Integer),
    ParamMeta::new("limit")
        .description("Maximum number of results (default: 10)")
        .param_type(ParamType::Integer),
];

impl Operation for SearchHistory {
    fn verb(&self) -> &'static str {
        "search"
    }
    fn noun(&self) -> &'static str {
        "history"
    }
    fn description(&self) -> &'static str {
        "Semantic search across all command output using embeddings. Finds content by meaning."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SEARCH_HISTORY_PARAMS
    }
}

/// Execute the "search history" operation.
///
/// Extracts the `query`, optional `command_id`, and optional `limit` parameters
/// from `args`, then performs a semantic search over stored command output.
/// The lock is released before the async search to avoid blocking other operations.
///
/// # Parameters
///
/// - `args`: the MCP argument map (without the "op" key)
/// - `state`: shared shell state containing command history and the search database
///
/// # Returns
///
/// A `CallToolResult` with formatted search results, or an `McpError` on failure.
pub async fn execute_search_history(
    args: &serde_json::Map<String, serde_json::Value>,
    state: Arc<Mutex<ShellState>>,
) -> Result<CallToolResult, McpError> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
        McpError::invalid_params("'query' parameter is required for search history", None)
    })?;
    let command_id = args
        .get("command_id")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    // Clone search data under lock, then release lock before the expensive async search
    let (session_id, db) = {
        let guard = state.lock().await;
        guard.search_handle()
    };
    // Lock is released — search runs without blocking other shell operations
    match state::search(&session_id, &db, query, command_id, limit).await {
        Ok((results, total)) => {
            if results.is_empty() {
                return Ok(BaseToolImpl::create_success_response(
                    "No matching results found.".to_string(),
                ));
            }
            let mut output = String::new();
            for r in &results {
                output.push_str(&format!(
                    "[cmd {}, lines {}-{}] (similarity: {:.2})\n{}\n\n",
                    r.command_id, r.start_line, r.end_line, r.similarity, r.text
                ));
            }
            if total > results.len() {
                output.push_str(&format!(
                    "Showing {} of {} total matches. Use 'limit' parameter to see more.\n",
                    results.len(),
                    total
                ));
            }
            Ok(BaseToolImpl::create_success_response(output))
        }
        Err(e) => Err(McpError::internal_error(
            format!("Search failed: {}", e),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::execute_op;

    // =====================================================================
    // Tests for "search history" operation
    // =====================================================================

    #[tokio::test]
    async fn test_search_history_missing_query_returns_error() {
        let result = execute_op("search history", vec![]).await;
        assert!(result.is_err(), "search history without query should fail");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("query"),
            "Error should mention 'query': {}",
            err_str
        );
    }
}
