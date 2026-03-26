//! List processes operation for the shell tool.
//!
//! This module implements the "list processes" operation which shows all commands
//! with their status, exit code, line count, start/stop times, and duration.

use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta};

use super::state::{CommandStatus, ShellState};
use crate::mcp::tool_registry::BaseToolImpl;

/// Operation metadata for listing all commands with status and timing
#[derive(Debug, Default)]
pub struct ListProcesses;

static LIST_PROCESSES_PARAMS: &[ParamMeta] = &[];

impl Operation for ListProcesses {
    fn verb(&self) -> &'static str {
        "list"
    }
    fn noun(&self) -> &'static str {
        "processes"
    }
    fn description(&self) -> &'static str {
        "Show all commands with status, exit code, line count, start/stop times, and duration"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        LIST_PROCESSES_PARAMS
    }
}

/// Execute the "list processes" operation.
///
/// Returns a formatted table of all commands in the shell state history,
/// including their ID, status, exit code, line count, start time, duration,
/// and the command string itself.
///
/// # Parameters
///
/// - `state`: shared shell state containing command history
///
/// # Returns
///
/// A `CallToolResult` with a formatted table string, or an `McpError` on failure.
pub async fn execute_list_processes(
    state: Arc<Mutex<ShellState>>,
) -> Result<CallToolResult, McpError> {
    let guard = state.lock().await;
    let commands = guard.list_commands();
    if commands.is_empty() {
        return Ok(BaseToolImpl::create_success_response(
            "No commands in history.".to_string(),
        ));
    }
    let mut output =
        String::from("ID  STATUS      EXIT  LINES  STARTED              DURATION  COMMAND\n");
    for cmd in commands {
        let duration = cmd.duration();
        let dur_str = if cmd.status == CommandStatus::Running {
            format!("{:.1}s+", duration.as_secs_f64())
        } else {
            format!("{:.1}s", duration.as_secs_f64())
        };
        let exit_str = cmd
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());
        output.push_str(&format!(
            "{:<3} {:<11} {:<5} {:<6} {}  {:<9} {}\n",
            cmd.id,
            cmd.status,
            exit_str,
            cmd.line_count,
            cmd.started_at_wall.format("%Y-%m-%d %H:%M:%S"),
            dur_str,
            cmd.command,
        ));
    }
    Ok(BaseToolImpl::create_success_response(output))
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{
        execute_op_with, extract_text, run_command_with, shared_tool,
    };
    use serial_test::serial;

    // =====================================================================
    // Tests for "list processes" operation
    // =====================================================================

    #[tokio::test]
    #[serial(cwd)]
    async fn test_list_processes_shows_completed_commands() {
        let tool = shared_tool();
        // Run a command first so there's something to list
        run_command_with(&tool, "echo list_test_marker").await;

        let result = execute_op_with(&tool, "list processes", vec![]).await;
        assert!(
            result.is_ok(),
            "list processes should succeed: {:?}",
            result.err()
        );

        let call_result = result.unwrap();
        let text = extract_text(&call_result);

        // Should contain table headers
        assert!(text.contains("ID"), "Should have ID column header");
        assert!(text.contains("STATUS"), "Should have STATUS column header");
        assert!(
            text.contains("COMMAND"),
            "Should have COMMAND column header"
        );

        // Should contain our command
        assert!(
            text.contains("echo list_test_marker"),
            "Should list the command we ran"
        );
        assert!(
            text.contains("completed"),
            "Command should show completed status"
        );
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_list_processes_table_format() {
        let tool = shared_tool();
        run_command_with(&tool, "echo format_check").await;

        let result = execute_op_with(&tool, "list processes", vec![]).await;
        let call_result = result.unwrap();
        let text = extract_text(&call_result);

        // Verify all expected columns are in the header
        let header_line = text.lines().next().expect("Should have at least one line");
        assert!(header_line.contains("ID"));
        assert!(header_line.contains("STATUS"));
        assert!(header_line.contains("EXIT"));
        assert!(header_line.contains("LINES"));
        assert!(header_line.contains("STARTED"));
        assert!(header_line.contains("DURATION"));
        assert!(header_line.contains("COMMAND"));
    }
}
