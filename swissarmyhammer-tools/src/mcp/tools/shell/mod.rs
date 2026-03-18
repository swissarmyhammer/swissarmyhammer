//! Shell tool for MCP — virtual shell with history, process management, and semantic search.
//!
//! Dispatches between six operations:
//! - `execute command`: Run a shell command with timeout and output capture
//! - `list processes`: Show all commands with status, timing, exit codes
//! - `kill process`: Stop a running command by ID
//! - `search history`: Semantic search across command output
//! - `grep history`: Regex pattern match across command output
//! - `get lines`: Retrieve specific lines from a command's output

pub mod execute_command;
pub mod get_lines;
pub mod grep_history;
pub mod infrastructure;
pub mod kill_process;
pub mod list_processes;
pub mod process;
pub mod search_history;
pub mod state;

#[cfg(test)]
pub(crate) mod test_helpers;

// Re-export public types from infrastructure
pub use infrastructure::{
    format_output_content, is_binary_content, OutputBuffer, OutputLimits, ShellError,
    ShellExecutionResult,
};

use crate::mcp::tool_registry::{McpTool, ToolContext};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::sync::Arc;
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};
use tokio::sync::Mutex;

use state::ShellState;

// Static operation instances for schema generation
static EXECUTE_CMD: Lazy<execute_command::ExecuteCommand> =
    Lazy::new(execute_command::ExecuteCommand::default);
static LIST_PROCS: Lazy<list_processes::ListProcesses> =
    Lazy::new(list_processes::ListProcesses::default);
static KILL_PROC: Lazy<kill_process::KillProcess> = Lazy::new(kill_process::KillProcess::default);
static SEARCH_HIST: Lazy<search_history::SearchHistory> =
    Lazy::new(search_history::SearchHistory::default);
static GREP_HIST: Lazy<grep_history::GrepHistory> = Lazy::new(grep_history::GrepHistory::default);
static GET_LNS: Lazy<get_lines::GetLines> = Lazy::new(get_lines::GetLines::default);

pub static SHELL_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*EXECUTE_CMD as &dyn Operation,
        &*LIST_PROCS as &dyn Operation,
        &*KILL_PROC as &dyn Operation,
        &*SEARCH_HIST as &dyn Operation,
        &*GREP_HIST as &dyn Operation,
        &*GET_LNS as &dyn Operation,
    ]
});

/// Tool for executing shell commands
#[derive(Clone)]
pub struct ShellExecuteTool {
    state: Arc<Mutex<ShellState>>,
}

impl Default for ShellExecuteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellExecuteTool {
    /// Creates a new instance of the ShellExecuteTool with in-memory state.
    pub fn new() -> Self {
        let state = ShellState::new().expect("Failed to initialize shell state");
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Creates an instance rooted in an isolated temp directory.
    ///
    /// Use this in tests to avoid depending on the process CWD, which can
    /// become invalid when concurrent tests delete their temp directories.
    #[cfg(test)]
    pub(crate) fn new_isolated() -> Self {
        let dir = std::env::temp_dir().join(format!(".shell-test-{}", ulid::Ulid::new()));
        let state = ShellState::with_dir(dir).expect("Failed to initialize isolated shell state");
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }
}

// No health checks needed
crate::impl_empty_doctorable!(ShellExecuteTool);
crate::impl_empty_initializable!(ShellExecuteTool);

#[async_trait]
impl McpTool for ShellExecuteTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let config = SchemaConfig::new(
            "Virtual shell with history, process management, and semantic search. Execute commands, search output history, grep patterns, and manage running processes.",
        );
        generate_mcp_schema(&SHELL_OPERATIONS, config)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&'static dyn Operation] = &SHELL_OPERATIONS;
        // SAFETY: SHELL_OPERATIONS is a static Lazy<Vec<...>> initialized once and lives for 'static
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
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        // Strip op from arguments before parsing
        let mut args = arguments.clone();
        args.remove("op");

        match op_str {
            "execute command" | "" => {
                return execute_command::execute_execute_command(
                    args,
                    self.state.clone(),
                    _context,
                )
                .await;
            }
            "list processes" => {
                return list_processes::execute_list_processes(self.state.clone()).await;
            }
            "kill process" => {
                return kill_process::execute_kill_process(&args, self.state.clone()).await;
            }
            "search history" => {
                return search_history::execute_search_history(&args, self.state.clone()).await;
            }
            "grep history" => {
                return grep_history::execute_grep_history(&args, self.state.clone()).await;
            }
            "get lines" => {
                return get_lines::execute_get_lines(&args, self.state.clone()).await;
            }
            other => {
                return Err(McpError::invalid_params(
                    format!(
                        "Unknown operation '{}'. Valid operations: execute command, list processes, kill process, search history, grep history, get lines",
                        other
                    ),
                    None,
                ));
            }
        }
    }
}

use crate::mcp::tool_registry::ToolRegistry;

/// Register all shell-related tools with the registry
///
/// This function registers all shell command execution tools following the
/// SwissArmyHammer tool registry pattern. Currently includes:
///
/// - `shell_execute`: Execute shell commands with timeout and environment control
///
/// # Arguments
///
/// * `registry` - The tool registry to register shell tools with
///
/// # Example
///
/// ```rust,ignore
/// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
/// use swissarmyhammer_tools::mcp::tools::shell::register_shell_tools;
///
/// let mut registry = ToolRegistry::new();
/// register_shell_tools(&mut registry);
/// ```
pub fn register_shell_tools(registry: &mut ToolRegistry) {
    registry.register(ShellExecuteTool::new());
}

/// Test-only variant that uses isolated temp dirs instead of CWD.
#[cfg(test)]
fn register_shell_tools_isolated(registry: &mut ToolRegistry) {
    registry.register(ShellExecuteTool::new_isolated());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    // Import test helpers
    use test_helpers::execute_op;

    // =====================================================================
    // Registration tests
    // =====================================================================

    #[tokio::test]
    async fn test_register_shell_tools() {
        let mut registry = ToolRegistry::new();
        register_shell_tools_isolated(&mut registry);

        // Verify shell_execute tool is registered
        assert!(registry.get_tool("shell").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_shell_tools_properties() {
        let mut registry = ToolRegistry::new();
        register_shell_tools_isolated(&mut registry);

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);

        let shell_execute_tool = tools
            .iter()
            .find(|tool| tool.name == "shell")
            .expect("shell_execute tool should be registered");

        assert_eq!(shell_execute_tool.name, "shell");
        assert!(shell_execute_tool.description.is_some());
        assert!(!shell_execute_tool.input_schema.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_registrations() {
        let mut registry = ToolRegistry::new();

        // Register twice to ensure no conflicts
        register_shell_tools_isolated(&mut registry);
        register_shell_tools_isolated(&mut registry);

        // Should have only one tool (second registration overwrites)
        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("shell").is_some());
    }

    #[tokio::test]
    async fn test_shell_tool_name_uniqueness() {
        let mut registry = ToolRegistry::new();
        register_shell_tools_isolated(&mut registry);

        let tool_names = registry.list_tool_names();
        let unique_names: std::collections::HashSet<_> = tool_names.iter().collect();

        // All tool names should be unique
        assert_eq!(tool_names.len(), unique_names.len());
    }

    // =====================================================================
    // Tool property tests
    // =====================================================================

    #[tokio::test]
    async fn test_shell_tool_has_operations() {
        let tool = ShellExecuteTool::new_isolated();
        let ops = tool.operations();
        assert_eq!(ops.len(), 6);
        assert!(ops.iter().any(|o| o.op_string() == "execute command"));
        assert!(ops.iter().any(|o| o.op_string() == "list processes"));
        assert!(ops.iter().any(|o| o.op_string() == "kill process"));
        assert!(ops.iter().any(|o| o.op_string() == "search history"));
        assert!(ops.iter().any(|o| o.op_string() == "grep history"));
        assert!(ops.iter().any(|o| o.op_string() == "get lines"));
    }

    #[tokio::test]
    async fn test_tool_properties() {
        let tool = ShellExecuteTool::new_isolated();
        assert_eq!(tool.name(), "shell");
        assert!(!tool.description().is_empty());

        let schema = tool.schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["command"].is_object());
        assert!(schema["properties"]["op"].is_object());
        assert!(schema["x-operation-schemas"].is_array());
        assert!(schema["x-operation-groups"].is_object());
    }

    // =====================================================================
    // Tests for unknown operations
    // =====================================================================

    #[tokio::test]
    async fn test_unknown_operation_returns_error() {
        let result = execute_op("bogus operation", vec![]).await;
        assert!(result.is_err(), "Unknown operation should fail");
        let err = result.unwrap_err();
        let err_str = err.to_string();
        assert!(
            err_str.contains("bogus operation"),
            "Error should echo the bad op: {}",
            err_str
        );
        assert!(
            err_str.contains("execute command"),
            "Error should list valid operations: {}",
            err_str
        );
    }

    #[tokio::test]
    async fn test_unknown_operation_lists_all_valid_ops() {
        let result = execute_op("not a real op", vec![]).await;
        let err = result.unwrap_err();
        let err_str = err.to_string();

        // Should list all valid operations
        for expected_op in &[
            "execute command",
            "list processes",
            "kill process",
            "search history",
            "grep history",
            "get lines",
        ] {
            assert!(
                err_str.contains(expected_op),
                "Error should list '{}': {}",
                expected_op,
                err_str
            );
        }
    }
}
