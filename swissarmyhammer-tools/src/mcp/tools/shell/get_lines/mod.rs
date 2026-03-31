//! Get lines operation for the shell tool.
//!
//! This module implements the "get lines" operation which retrieves specific
//! lines from a command's stored output by line number range.

use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

use super::infrastructure::value_as_u64_tolerant;
use super::state::ShellState;
use crate::mcp::tool_registry::BaseToolImpl;

/// Operation metadata for retrieving specific lines from a command's output
#[derive(Debug, Default)]
pub struct GetLines;

static GET_LINES_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("command_id")
        .description("Which command's output to retrieve lines from")
        .param_type(ParamType::Integer)
        .required(),
    ParamMeta::new("start")
        .description("Start line number (default: 1)")
        .param_type(ParamType::Integer),
    ParamMeta::new("end")
        .description("End line number (default: last line)")
        .param_type(ParamType::Integer),
];

impl Operation for GetLines {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "lines"
    }
    fn description(&self) -> &'static str {
        "Retrieve specific lines from a command's output by range"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_LINES_PARAMS
    }
}

/// Execute the "get lines" operation.
///
/// Extracts `command_id` (required), optional `start`, and optional `end`
/// parameters from `args`, then retrieves the stored output lines for that
/// command, returning them with line-number prefixes.
///
/// # Parameters
///
/// - `args`: the MCP argument map (without the "op" key)
/// - `state`: shared shell state containing the command history store
///
/// # Returns
///
/// A `CallToolResult` with the formatted lines, or an `McpError` on failure.
pub async fn execute_get_lines(
    args: &serde_json::Map<String, serde_json::Value>,
    state: Arc<Mutex<ShellState>>,
) -> Result<CallToolResult, McpError> {
    let command_id = args
        .get("command_id")
        .and_then(value_as_u64_tolerant)
        .ok_or_else(|| {
            McpError::invalid_params("'command_id' parameter is required for get lines", None)
        })? as usize;
    let start = args
        .get("start")
        .and_then(value_as_u64_tolerant)
        .map(|v| v as usize);
    let end = args
        .get("end")
        .and_then(value_as_u64_tolerant)
        .map(|v| v as usize);

    let guard = state.lock().await;
    match guard.get_lines(command_id, start, end) {
        Ok(lines) => {
            if lines.is_empty() {
                return Ok(BaseToolImpl::create_success_response(format!(
                    "No output lines found for command {}.",
                    command_id
                )));
            }
            let start_line = lines.first().map(|(n, _)| *n).unwrap_or(0);
            let end_line = lines.last().map(|(n, _)| *n).unwrap_or(0);
            let mut output = format!("[cmd {}, lines {}-{}]\n", command_id, start_line, end_line);
            for (num, text) in &lines {
                output.push_str(&format!("{}: {}\n", num, text));
            }
            Ok(BaseToolImpl::create_success_response(output))
        }
        Err(e) => Err(McpError::internal_error(
            format!("Get lines failed: {}", e),
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
    // Tests for "get lines" operation
    // =====================================================================

    #[tokio::test]
    async fn test_get_lines_missing_command_id_returns_error() {
        let result = execute_op("get lines", vec![]).await;
        assert!(result.is_err(), "get lines without command_id should fail");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("command_id"),
            "Error should mention 'command_id': {}",
            err_str
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_get_lines_retrieves_output() {
        let tool = shared_tool();
        let cmd_id = run_command_with(&tool, "echo 'GET_LINES_OUTPUT'").await;

        let result = execute_op_with(&tool, "get lines", vec![("command_id", json!(cmd_id))]).await;
        assert!(
            result.is_ok(),
            "get lines should succeed: {:?}",
            result.err()
        );

        let text = extract_text(&result.unwrap());
        assert!(
            text.contains("GET_LINES_OUTPUT"),
            "Should contain command output: {}",
            text
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_get_lines_with_range() {
        let tool = shared_tool();
        // Run a command that produces multiple lines
        let cmd_id =
            run_command_with(&tool, "printf 'line1\\nline2\\nline3\\nline4\\nline5\\n'").await;

        // Get only lines 2-4
        let result = execute_op_with(
            &tool,
            "get lines",
            vec![
                ("command_id", json!(cmd_id)),
                ("start", json!(2)),
                ("end", json!(4)),
            ],
        )
        .await;
        assert!(result.is_ok(), "get lines with range should succeed");

        let text = extract_text(&result.unwrap());
        assert!(text.contains("line2"), "Should contain line2: {}", text);
        assert!(text.contains("line4"), "Should contain line4: {}", text);
        // line1 should not be present (before start)
        assert!(
            !text.contains("line1"),
            "Should not contain line1 (before range): {}",
            text
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_get_lines_nonexistent_command() {
        let result = execute_op("get lines", vec![("command_id", json!(99999))]).await;
        assert!(
            result.is_ok(),
            "get lines for missing command should succeed with empty"
        );

        let text = extract_text(&result.unwrap());
        assert!(
            text.contains("No output lines"),
            "Should report no lines: {}",
            text
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_get_lines_shows_line_numbers() {
        let tool = shared_tool();
        let cmd_id = run_command_with(&tool, "printf 'alpha\\nbeta\\ngamma\\n'").await;

        let result = execute_op_with(&tool, "get lines", vec![("command_id", json!(cmd_id))]).await;
        let text = extract_text(&result.unwrap());

        // Should include line numbers in the output
        assert!(
            text.contains("1:") || text.contains("1: "),
            "Should show line numbers: {}",
            text
        );
    }
}
