//! Grep history operation for the shell tool.
//!
//! This module implements the "grep history" operation which performs
//! regex/literal pattern matching across all command output history,
//! finding content by exact structural match.

use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

use super::state::ShellState;
use crate::mcp::tool_registry::BaseToolImpl;

/// Operation metadata for regex/literal pattern matching on command output history
#[derive(Debug, Default)]
pub struct GrepHistory;

static GREP_HISTORY_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("pattern")
        .description("Regex pattern to match against command output")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("command_id")
        .description("Filter to a specific command's output (optional)")
        .param_type(ParamType::Integer),
    ParamMeta::new("limit")
        .description("Maximum number of results (default: 50)")
        .param_type(ParamType::Integer),
];

impl Operation for GrepHistory {
    fn verb(&self) -> &'static str {
        "grep"
    }
    fn noun(&self) -> &'static str {
        "history"
    }
    fn description(&self) -> &'static str {
        "Regex pattern match across command output history. Exact structural search."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GREP_HISTORY_PARAMS
    }
}

/// Execute the "grep history" operation.
///
/// Extracts the `pattern`, optional `command_id`, and optional `limit` parameters
/// from `args`, then performs a regex match over stored command output.
///
/// # Parameters
///
/// - `args`: the MCP argument map (without the "op" key)
/// - `state`: shared shell state containing command history
///
/// # Returns
///
/// A `CallToolResult` with formatted grep results, or an `McpError` on failure.
pub async fn execute_grep_history(
    args: &serde_json::Map<String, serde_json::Value>,
    state: Arc<Mutex<ShellState>>,
) -> Result<CallToolResult, McpError> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            McpError::invalid_params("'pattern' parameter is required for grep history", None)
        })?;
    let command_id = args
        .get("command_id")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let guard = state.lock().await;
    match guard.grep(pattern, command_id, limit) {
        Ok((results, total)) => {
            if results.is_empty() {
                return Ok(BaseToolImpl::create_success_response(
                    "No matching results found.".to_string(),
                ));
            }
            let mut output = String::new();
            for r in &results {
                output.push_str(&format!(
                    "[cmd {}, line {}] {}\n",
                    r.command_id, r.line_number, r.text
                ));
            }
            if total > results.len() {
                output.push_str(&format!(
                    "\nShowing {} of {} total matches. Use 'limit' parameter to see more.\n",
                    results.len(),
                    total
                ));
            }
            Ok(BaseToolImpl::create_success_response(output))
        }
        Err(e) => Err(McpError::internal_error(
            format!("Grep failed: {}", e),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use serial_test::serial;

    use super::super::test_helpers::{
        execute_op, execute_op_with, extract_text, run_command_with, shared_tool,
    };

    // =====================================================================
    // Tests for "grep history" operation
    // =====================================================================

    #[tokio::test]
    async fn test_grep_history_missing_pattern_returns_error() {
        let result = execute_op("grep history", vec![]).await;
        assert!(result.is_err(), "grep history without pattern should fail");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("pattern"),
            "Error should mention 'pattern': {}",
            err_str
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_grep_history_finds_matching_output() {
        let tool = shared_tool();
        // Run a command that produces known output
        run_command_with(&tool, "echo UNIQUE_GREP_MARKER_12345").await;

        let result = execute_op_with(
            &tool,
            "grep history",
            vec![("pattern", json!("UNIQUE_GREP_MARKER_12345"))],
        )
        .await;
        assert!(result.is_ok(), "grep should succeed: {:?}", result.err());

        let text = extract_text(&result.unwrap());
        assert!(
            text.contains("UNIQUE_GREP_MARKER_12345"),
            "Should find the marker in results: {}",
            text
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_grep_history_no_matches() {
        let result = execute_op(
            "grep history",
            vec![("pattern", json!("ABSOLUTELY_IMPOSSIBLE_PATTERN_XYZZY_999"))],
        )
        .await;
        assert!(result.is_ok(), "grep with no matches should succeed");

        let text = extract_text(&result.unwrap());
        assert!(
            text.contains("No matching results"),
            "Should report no matches: {}",
            text
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_grep_history_with_command_id_filter() {
        let tool = shared_tool();
        let cmd_id = run_command_with(&tool, "echo GREP_FILTER_TARGET").await;

        let result = execute_op_with(
            &tool,
            "grep history",
            vec![
                ("pattern", json!("GREP_FILTER_TARGET")),
                ("command_id", json!(cmd_id)),
            ],
        )
        .await;
        assert!(result.is_ok(), "grep with command_id filter should succeed");

        let text = extract_text(&result.unwrap());
        assert!(
            text.contains("GREP_FILTER_TARGET"),
            "Should find match in filtered command: {}",
            text
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_grep_history_with_limit() {
        let tool = shared_tool();
        // Run a command with multiple matching lines
        run_command_with(
            &tool,
            "printf 'LIMIT_LINE\\nLIMIT_LINE\\nLIMIT_LINE\\nLIMIT_LINE\\nLIMIT_LINE\\n'",
        )
        .await;

        let result = execute_op_with(
            &tool,
            "grep history",
            vec![("pattern", json!("LIMIT_LINE")), ("limit", json!(2))],
        )
        .await;
        assert!(result.is_ok(), "grep with limit should succeed");

        let text = extract_text(&result.unwrap());
        // Count occurrences of the pattern marker in results
        let count = text.matches("LIMIT_LINE").count();
        assert!(
            count <= 2,
            "Should respect limit of 2, got {} matches",
            count
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_grep_history_regex_pattern() {
        let tool = shared_tool();
        run_command_with(&tool, "echo 'error: something failed at line 42'").await;

        let result = execute_op_with(
            &tool,
            "grep history",
            vec![("pattern", json!("error:.*line \\d+"))],
        )
        .await;
        assert!(result.is_ok(), "regex grep should succeed");

        let text = extract_text(&result.unwrap());
        assert!(text.contains("error:"), "Should find regex match: {}", text);
    }

    #[tokio::test]
    async fn test_grep_history_invalid_regex_returns_error() {
        let result = execute_op("grep history", vec![("pattern", json!("[invalid regex"))]).await;
        assert!(result.is_err(), "invalid regex should fail");
    }
}
