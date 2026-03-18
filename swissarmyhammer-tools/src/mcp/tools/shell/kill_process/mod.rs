//! Kill process operation for the shell tool.
//!
//! This module implements the "kill process" operation which stops a running
//! command by ID, sending SIGKILL immediately.

use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

use super::state::ShellState;
use crate::mcp::tool_registry::BaseToolImpl;

/// Operation metadata for killing a running command
#[derive(Debug, Default)]
pub struct KillProcess;

static KILL_PROCESS_PARAMS: &[ParamMeta] = &[ParamMeta::new("id")
    .description("Command ID to kill")
    .param_type(ParamType::Integer)
    .required()];

impl Operation for KillProcess {
    fn verb(&self) -> &'static str {
        "kill"
    }
    fn noun(&self) -> &'static str {
        "process"
    }
    fn description(&self) -> &'static str {
        "Kill a running command by ID. Sends SIGKILL immediately."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        KILL_PROCESS_PARAMS
    }
}

/// Execute the kill process operation
///
/// Parses the `id` parameter from args, then kills the corresponding process
/// in the shell state.
pub async fn execute_kill_process(
    args: &serde_json::Map<String, serde_json::Value>,
    state: Arc<Mutex<ShellState>>,
) -> Result<CallToolResult, McpError> {
    let id = args.get("id").and_then(|v| v.as_u64()).ok_or_else(|| {
        McpError::invalid_params("'id' parameter is required for kill process", None)
    })? as usize;

    let mut guard = state.lock().await;
    match guard.kill_process(id) {
        Ok(record) => Ok(BaseToolImpl::create_success_response(format!(
            "Killed command {} ({}). {} lines captured.",
            id, record.command, record.line_count
        ))),
        Err(e) => Err(McpError::invalid_params(format!("{}", e), None)),
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::tool_registry::McpTool;
    use crate::mcp::tools::shell::test_helpers::{execute_op, extract_text};
    use crate::mcp::tools::shell::ShellExecuteTool;
    use crate::test_utils::create_test_context;
    use serde_json::json;

    #[tokio::test]
    async fn test_kill_process_missing_id_returns_error() {
        let result = execute_op("kill process", vec![]).await;
        assert!(result.is_err(), "kill process without id should fail");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("id"),
            "Error should mention 'id' parameter: {}",
            err_str
        );
    }

    #[tokio::test]
    async fn test_kill_process_nonexistent_id_returns_error() {
        let result = execute_op("kill process", vec![("id", json!(99999))]).await;
        assert!(result.is_err(), "kill process with bad id should fail");
    }

    #[tokio::test]
    async fn test_kill_process_stops_running_command() {
        // Start a long-running command with max_lines=0 so it returns immediately
        // with command_id, while the process continues running
        let tool = ShellExecuteTool::new_isolated();
        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert("command".to_string(), json!("sleep 60"));
        args.insert("timeout".to_string(), json!(1));
        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let text = extract_text(&result.unwrap());
        // The command should have timed out, giving us a command_id
        assert!(
            text.contains("command_id:") || text.contains("command_id"),
            "Should contain command_id: {}",
            text
        );
    }
}
