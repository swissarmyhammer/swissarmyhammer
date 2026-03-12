//! Ralph MCP tool implementation
//!
//! Implements the `McpTool` trait for persistent agent loop instructions.
//! Stores per-session instructions as markdown files in `.sah/ralph/`.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{generate_mcp_schema, Operation, ParamMeta, ParamType, SchemaConfig};

use super::state::{delete_ralph, read_ralph, write_ralph, RalphState};

// --- Operation metadata ---

/// Set ralph instruction for a session
#[derive(Debug, Default)]
pub struct SetRalph;

static SET_RALPH_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("session_id")
        .description("Session ID to set instruction for (defaults to current MCP session)")
        .param_type(ParamType::String),
    ParamMeta::new("instruction")
        .description("Instruction text to persist (used as the ongoing goal)")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("max_iterations")
        .description("Maximum iterations before auto-stop (default: 50)")
        .param_type(ParamType::Integer),
    ParamMeta::new("body")
        .description("Optional notes/context to store in the file body")
        .param_type(ParamType::String),
];

impl Operation for SetRalph {
    fn verb(&self) -> &'static str {
        "set"
    }
    fn noun(&self) -> &'static str {
        "ralph"
    }
    fn description(&self) -> &'static str {
        "Store a persistent instruction for a session. Creates .sah/ralph/<session_id>.md"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SET_RALPH_PARAMS
    }
}

/// Check if a session has an active ralph instruction (Stop hook responder)
#[derive(Debug, Default)]
pub struct CheckRalph;

static CHECK_RALPH_PARAMS: &[ParamMeta] = &[ParamMeta::new("session_id")
    .description("Session ID to check for active instructions")
    .param_type(ParamType::String)
    .required()];

impl Operation for CheckRalph {
    fn verb(&self) -> &'static str {
        "check"
    }
    fn noun(&self) -> &'static str {
        "ralph"
    }
    fn description(&self) -> &'static str {
        "Check if a session has an active instruction. Returns block/allow JSON for Stop hook integration."
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        CHECK_RALPH_PARAMS
    }
}

/// Clear a session's ralph instruction
#[derive(Debug, Default)]
pub struct ClearRalph;

static CLEAR_RALPH_PARAMS: &[ParamMeta] = &[ParamMeta::new("session_id")
    .description("Session ID to clear instruction for (defaults to current MCP session)")
    .param_type(ParamType::String)];

impl Operation for ClearRalph {
    fn verb(&self) -> &'static str {
        "clear"
    }
    fn noun(&self) -> &'static str {
        "ralph"
    }
    fn description(&self) -> &'static str {
        "Remove a session's persistent instruction. Deletes .sah/ralph/<session_id>.md"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        CLEAR_RALPH_PARAMS
    }
}

/// Get a session's ralph instruction content
#[derive(Debug, Default)]
pub struct GetRalph;

static GET_RALPH_PARAMS: &[ParamMeta] = &[ParamMeta::new("session_id")
    .description("Session ID to get instruction for (defaults to current MCP session)")
    .param_type(ParamType::String)];

impl Operation for GetRalph {
    fn verb(&self) -> &'static str {
        "get"
    }
    fn noun(&self) -> &'static str {
        "ralph"
    }
    fn description(&self) -> &'static str {
        "Read a session's persistent instruction content"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GET_RALPH_PARAMS
    }
}

// --- Static operation instances ---

static SET_OP: Lazy<SetRalph> = Lazy::new(SetRalph::default);
static CHECK_OP: Lazy<CheckRalph> = Lazy::new(CheckRalph::default);
static CLEAR_OP: Lazy<ClearRalph> = Lazy::new(ClearRalph::default);
static GET_OP: Lazy<GetRalph> = Lazy::new(GetRalph::default);

/// All ralph operations exposed for schema generation and CLI discovery
pub static RALPH_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*SET_OP as &dyn Operation,
        &*CHECK_OP as &dyn Operation,
        &*CLEAR_OP as &dyn Operation,
        &*GET_OP as &dyn Operation,
    ]
});

/// Ralph MCP tool for persistent agent loop instructions
///
/// Stores per-session instructions as markdown files with YAML frontmatter
/// in `.sah/ralph/<session_id>.md`. Used by Stop hooks to prevent Claude
/// from stopping while work remains.
#[derive(Default)]
pub struct RalphTool;

impl RalphTool {
    /// Create a new RalphTool instance
    pub fn new() -> Self {
        Self
    }
}

impl swissarmyhammer_common::health::Doctorable for RalphTool {
    fn name(&self) -> &str {
        "Ralph"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<swissarmyhammer_common::health::HealthCheck> {
        // Directory existence is verified lazily on write; no external deps to check
        Vec::new()
    }

    fn is_applicable(&self) -> bool {
        true
    }
}

impl swissarmyhammer_common::lifecycle::Initializable for RalphTool {
    fn name(&self) -> &str {
        "ralph"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn init(
        &self,
        _scope: &swissarmyhammer_common::lifecycle::InitScope,
    ) -> Vec<swissarmyhammer_common::lifecycle::InitResult> {
        use super::state::ensure_ralph_dir;
        use swissarmyhammer_common::lifecycle::InitResult;

        // Create .sah/ralph/ eagerly on init rather than lazily on first write
        match ensure_ralph_dir(std::path::Path::new(".")) {
            Ok(()) => vec![InitResult::ok("ralph", "Created .sah/ralph/ directory")],
            Err(e) => vec![InitResult::error(
                "ralph",
                format!("Failed to create .sah/ralph/: {e}"),
            )],
        }
    }
}

#[async_trait]
impl McpTool for RalphTool {
    fn name(&self) -> &'static str {
        "ralph"
    }

    fn description(&self) -> &'static str {
        include_str!("../description.md")
    }

    fn schema(&self) -> serde_json::Value {
        let config = SchemaConfig::new(
            "Persistent agent loop instructions with per-session state. Stores instructions as .sah/ralph/<session_id>.md files for Stop hook integration.",
        );
        generate_mcp_schema(&RALPH_OPERATIONS, config)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&dyn Operation] = &RALPH_OPERATIONS;
        // SAFETY: RALPH_OPERATIONS is a static Lazy<Vec<...>> initialized once and never dropped.
        // The references inside are also to static Lazy values with 'static lifetime.
        unsafe { std::mem::transmute(ops) }
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("ralph")
    }

    fn cli_name(&self) -> &'static str {
        "ralph"
    }

    fn is_agent_tool(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let op_str = arguments.get("op").and_then(|v| v.as_str()).unwrap_or("");

        let mut args = arguments.clone();
        args.remove("op");

        let base_dir = context
            .working_dir
            .as_deref()
            .unwrap_or(std::path::Path::new("."));

        match op_str {
            "set ralph" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| context.session_id.clone());
                let session_id = session_id.as_str();

                let instruction = args
                    .get("instruction")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("instruction is required", None))?;

                let max_iterations = args
                    .get("max_iterations")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(50);

                let body = args
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Preserve iteration counter from existing state to prevent safety cap bypass
                let existing_iteration = read_ralph(base_dir, session_id)
                    .map_err(|e| {
                        McpError::internal_error(format!("Failed to read ralph: {e}"), None)
                    })?
                    .map(|s| s.iteration)
                    .unwrap_or(0);

                let state = RalphState {
                    instruction: instruction.to_string(),
                    iteration: existing_iteration,
                    max_iterations,
                    body,
                };

                write_ralph(base_dir, session_id, &state).map_err(|e| {
                    McpError::internal_error(format!("Failed to set ralph: {e}"), None)
                })?;

                let response = serde_json::json!({
                    "session_id": session_id,
                    "instruction": instruction,
                    "iteration": existing_iteration,
                    "max_iterations": max_iterations,
                });
                Ok(BaseToolImpl::create_success_response(response.to_string()))
            }
            "check ralph" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("session_id is required", None))?;

                let state_result = read_ralph(base_dir, session_id).map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to read ralph state: {e}"),
                        None,
                    )
                })?;

                match state_result {
                    Some(mut state) => {
                        // Increment iteration counter
                        state.iteration += 1;

                        // Check if max iterations reached
                        if state.iteration > state.max_iterations {
                            // Auto-stop: delete file and allow
                            delete_ralph(base_dir, session_id).map_err(|e| {
                                McpError::internal_error(
                                    format!("Failed to clear ralph after max iterations: {e}"),
                                    None,
                                )
                            })?;
                            let response = serde_json::json!({
                                "decision": "allow",
                                "reason": format!(
                                    "Max iterations reached ({}/{}). Ralph auto-cleared.",
                                    state.iteration, state.max_iterations
                                ),
                            });
                            return Ok(BaseToolImpl::create_success_response(
                                response.to_string(),
                            ));
                        }

                        // Persist incremented iteration
                        write_ralph(base_dir, session_id, &state).map_err(|e| {
                            McpError::internal_error(
                                format!("Failed to update ralph iteration: {e}"),
                                None,
                            )
                        })?;

                        let response = serde_json::json!({
                            "decision": "block",
                            "reason": format!(
                                "{}. Iteration {} of {}.",
                                state.instruction, state.iteration, state.max_iterations
                            ),
                            "iteration": state.iteration,
                            "max_iterations": state.max_iterations,
                        });
                        Ok(BaseToolImpl::create_success_response(response.to_string()))
                    }
                    None => {
                        let response = serde_json::json!({
                            "decision": "allow"
                        });
                        Ok(BaseToolImpl::create_success_response(response.to_string()))
                    }
                }
            }
            "clear ralph" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| context.session_id.clone());
                let session_id = session_id.as_str();

                // Read state before deleting to include final iteration count
                let final_state = read_ralph(base_dir, session_id).map_err(|e| {
                    McpError::internal_error(format!("Failed to read ralph: {e}"), None)
                })?;

                delete_ralph(base_dir, session_id).map_err(|e| {
                    McpError::internal_error(format!("Failed to clear ralph: {e}"), None)
                })?;

                let response = match final_state {
                    Some(state) => serde_json::json!({
                        "cleared": true,
                        "session_id": session_id,
                        "final_iteration": state.iteration,
                        "max_iterations": state.max_iterations,
                    }),
                    None => serde_json::json!({
                        "cleared": false,
                        "session_id": session_id,
                        "message": "No active instruction found",
                    }),
                };

                Ok(BaseToolImpl::create_success_response(response.to_string()))
            }
            "get ralph" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| context.session_id.clone());
                let session_id = session_id.as_str();

                let state_result = read_ralph(base_dir, session_id).map_err(|e| {
                    McpError::internal_error(format!("Failed to read ralph: {e}"), None)
                })?;

                match state_result {
                    Some(state) => {
                        let response = serde_json::json!({
                            "active": true,
                            "instruction": state.instruction,
                            "iteration": state.iteration,
                            "max_iterations": state.max_iterations,
                            "body": state.body,
                        });
                        Ok(BaseToolImpl::create_success_response(response.to_string()))
                    }
                    None => {
                        let response = serde_json::json!({
                            "active": false,
                            "session_id": session_id,
                        });
                        Ok(BaseToolImpl::create_success_response(response.to_string()))
                    }
                }
            }
            other => Err(McpError::invalid_params(
                format!(
                    "Unknown operation: '{other}'. Valid operations: set ralph, check ralph, clear ralph, get ralph"
                ),
                None,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;
    use tempfile::TempDir;

    async fn make_context(tmp: &TempDir) -> ToolContext {
        let mut ctx = crate::test_utils::create_test_context().await;
        ctx.working_dir = Some(tmp.path().to_path_buf());
        ctx
    }

    #[tokio::test]
    async fn test_register_ralph_tool_directly() {
        let mut registry = ToolRegistry::new();
        registry.register(RalphTool::new());
        assert!(registry.get_tool("ralph").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_ralph_tool_properties() {
        let mut registry = ToolRegistry::new();
        registry.register(RalphTool::new());

        let tools = registry.list_tools();
        let ralph_tool = tools
            .iter()
            .find(|t| t.name == "ralph")
            .expect("ralph tool should be registered");
        assert_eq!(ralph_tool.name, "ralph");
        assert!(ralph_tool.description.is_some());
        assert!(!ralph_tool.input_schema.is_empty());
    }

    #[test]
    fn test_tool_name() {
        let tool = RalphTool::new();
        assert_eq!(tool.name(), "ralph");
    }

    #[test]
    fn test_cli_category() {
        let tool = RalphTool::new();
        assert_eq!(tool.cli_category(), Some("ralph"));
    }

    #[test]
    fn test_cli_name() {
        let tool = RalphTool::new();
        assert_eq!(tool.cli_name(), "ralph");
    }

    #[test]
    fn test_is_not_agent_tool() {
        let tool = RalphTool::new();
        assert!(!tool.is_agent_tool());
    }

    #[test]
    fn test_operation_metadata() {
        let set = SetRalph;
        assert_eq!(set.verb(), "set");
        assert_eq!(set.noun(), "ralph");
        assert_eq!(set.op_string(), "set ralph");

        let check = CheckRalph;
        assert_eq!(check.verb(), "check");
        assert_eq!(check.noun(), "ralph");
        assert_eq!(check.op_string(), "check ralph");

        let clear = ClearRalph;
        assert_eq!(clear.verb(), "clear");
        assert_eq!(clear.noun(), "ralph");
        assert_eq!(clear.op_string(), "clear ralph");

        let get = GetRalph;
        assert_eq!(get.verb(), "get");
        assert_eq!(get.noun(), "ralph");
        assert_eq!(get.op_string(), "get ralph");
    }

    #[test]
    fn test_operations_count() {
        assert_eq!(RALPH_OPERATIONS.len(), 4);
    }

    #[test]
    fn test_schema_generation() {
        let tool = RalphTool::new();
        let schema = tool.schema();
        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("properties"));
        // Verify op enum is present
        let props = obj["properties"].as_object().unwrap();
        assert!(props.contains_key("op"));
    }

    #[tokio::test]
    async fn test_set_ralph_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("set ralph"));
        args.insert(
            "session_id".to_string(),
            serde_json::json!("test-session"),
        );
        args.insert(
            "instruction".to_string(),
            serde_json::json!("Keep going until all cards are done"),
        );

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(!result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn test_check_ralph_allows_when_no_instruction() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("check ralph"));
        args.insert("session_id".to_string(), serde_json::json!("no-session"));

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(!result.is_error.unwrap_or(false));

        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["decision"], "allow");
    }

    #[tokio::test]
    async fn test_check_ralph_blocks_when_instruction_set() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set instruction
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("session-x"));
        set_args.insert(
            "instruction".to_string(),
            serde_json::json!("Keep working"),
        );
        tool.execute(set_args, &ctx).await.unwrap();

        // Check
        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("session-x"));

        let result = tool.execute(check_args, &ctx).await.unwrap();
        assert!(!result.is_error.unwrap_or(false));

        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["decision"], "block");
        // Reason includes instruction + iteration info
        let reason = json["reason"].as_str().unwrap();
        assert!(reason.contains("Keep working"));
        assert!(reason.contains("Iteration 1 of 50"));
    }

    #[tokio::test]
    async fn test_clear_ralph_execution() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set then clear
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("session-y"));
        set_args.insert("instruction".to_string(), serde_json::json!("test"));
        tool.execute(set_args, &ctx).await.unwrap();

        let mut clear_args = serde_json::Map::new();
        clear_args.insert("op".to_string(), serde_json::json!("clear ralph"));
        clear_args.insert("session_id".to_string(), serde_json::json!("session-y"));
        let result = tool.execute(clear_args, &ctx).await.unwrap();
        assert!(!result.is_error.unwrap_or(false));

        // Verify cleared
        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("session-y"));
        let check_result = tool.execute(check_args, &ctx).await.unwrap();
        let content = check_result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["decision"], "allow");
    }

    #[tokio::test]
    async fn test_clear_ralph_returns_final_iteration() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set instruction with custom max_iterations
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("session-iter"));
        set_args.insert("instruction".to_string(), serde_json::json!("test"));
        set_args.insert("max_iterations".to_string(), serde_json::json!(25));
        tool.execute(set_args, &ctx).await.unwrap();

        // Clear and check response includes iteration info
        let mut clear_args = serde_json::Map::new();
        clear_args.insert("op".to_string(), serde_json::json!("clear ralph"));
        clear_args.insert("session_id".to_string(), serde_json::json!("session-iter"));
        let result = tool.execute(clear_args, &ctx).await.unwrap();
        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["cleared"], true);
        assert_eq!(json["final_iteration"], 0);
        assert_eq!(json["max_iterations"], 25);
    }

    #[tokio::test]
    async fn test_get_ralph_returns_inactive_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get ralph"));
        args.insert("session_id".to_string(), serde_json::json!("ghost"));

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(!result.is_error.unwrap_or(false));
        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["active"], false);
    }

    #[tokio::test]
    async fn test_get_ralph_returns_active_with_details() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("session-get"));
        set_args.insert(
            "instruction".to_string(),
            serde_json::json!("Keep working"),
        );
        set_args.insert("max_iterations".to_string(), serde_json::json!(30));
        tool.execute(set_args, &ctx).await.unwrap();

        // Get
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), serde_json::json!("get ralph"));
        get_args.insert("session_id".to_string(), serde_json::json!("session-get"));
        let result = tool.execute(get_args, &ctx).await.unwrap();
        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["active"], true);
        assert_eq!(json["instruction"], "Keep working");
        assert_eq!(json["iteration"], 0);
        assert_eq!(json["max_iterations"], 30);
    }

    #[tokio::test]
    async fn test_set_replaces_previous_instruction() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set first instruction
        let mut args1 = serde_json::Map::new();
        args1.insert("op".to_string(), serde_json::json!("set ralph"));
        args1.insert("session_id".to_string(), serde_json::json!("session-replace"));
        args1.insert("instruction".to_string(), serde_json::json!("First instruction"));
        tool.execute(args1, &ctx).await.unwrap();

        // Replace with second
        let mut args2 = serde_json::Map::new();
        args2.insert("op".to_string(), serde_json::json!("set ralph"));
        args2.insert("session_id".to_string(), serde_json::json!("session-replace"));
        args2.insert("instruction".to_string(), serde_json::json!("Second instruction"));
        args2.insert("max_iterations".to_string(), serde_json::json!(99));
        tool.execute(args2, &ctx).await.unwrap();

        // Verify replacement
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), serde_json::json!("get ralph"));
        get_args.insert("session_id".to_string(), serde_json::json!("session-replace"));
        let result = tool.execute(get_args, &ctx).await.unwrap();
        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["instruction"], "Second instruction");
        assert_eq!(json["max_iterations"], 99);
        // Iteration is preserved from the previous state (was 0, stays 0)
        assert_eq!(json["iteration"], 0);
    }

    #[tokio::test]
    async fn test_set_ralph_preserves_iteration_counter() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set initial instruction
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("preserve-iter"));
        set_args.insert("instruction".to_string(), serde_json::json!("First"));
        tool.execute(set_args, &ctx).await.unwrap();

        // Increment iteration via check (3 times)
        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("preserve-iter"));
        tool.execute(check_args.clone(), &ctx).await.unwrap();
        tool.execute(check_args.clone(), &ctx).await.unwrap();
        tool.execute(check_args, &ctx).await.unwrap();

        // Re-set with new instruction — iteration should be preserved
        let mut reset_args = serde_json::Map::new();
        reset_args.insert("op".to_string(), serde_json::json!("set ralph"));
        reset_args.insert("session_id".to_string(), serde_json::json!("preserve-iter"));
        reset_args.insert("instruction".to_string(), serde_json::json!("Second"));
        let result = tool.execute(reset_args, &ctx).await.unwrap();
        let content = result.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap();
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        // Response should indicate the preserved iteration
        assert_eq!(json["iteration"], 3);

        // Verify via get
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), serde_json::json!("get ralph"));
        get_args.insert("session_id".to_string(), serde_json::json!("preserve-iter"));
        let get_result = tool.execute(get_args, &ctx).await.unwrap();
        let get_content = get_result.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap();
        let get_json: serde_json::Value = serde_json::from_str(get_content).unwrap();
        assert_eq!(get_json["instruction"], "Second");
        assert_eq!(get_json["iteration"], 3);
    }

    #[tokio::test]
    async fn test_custom_max_iterations_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("set ralph"));
        args.insert("session_id".to_string(), serde_json::json!("session-max"));
        args.insert("instruction".to_string(), serde_json::json!("test"));
        args.insert("max_iterations".to_string(), serde_json::json!(100));
        tool.execute(args, &ctx).await.unwrap();

        // Check persists through check
        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("session-max"));
        let result = tool.execute(check_args, &ctx).await.unwrap();
        let content = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["max_iterations"], 100);
    }

    #[tokio::test]
    async fn test_unknown_operation_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("fly ralph"));

        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }

    // --- RALPH-2: check operation iteration + max_iterations tests ---

    #[tokio::test]
    async fn test_check_increments_iteration_in_file() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set instruction
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("iter-test"));
        set_args.insert("instruction".to_string(), serde_json::json!("Keep going"));
        tool.execute(set_args, &ctx).await.unwrap();

        // Check once — iteration should go from 0 to 1
        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("iter-test"));
        tool.execute(check_args.clone(), &ctx).await.unwrap();

        // Verify iteration persisted to file
        let state = read_ralph(tmp.path(), "iter-test").unwrap().unwrap();
        assert_eq!(state.iteration, 1);

        // Check again — iteration should go to 2
        tool.execute(check_args.clone(), &ctx).await.unwrap();
        let state = read_ralph(tmp.path(), "iter-test").unwrap().unwrap();
        assert_eq!(state.iteration, 2);

        // Check a third time — iteration should go to 3
        tool.execute(check_args, &ctx).await.unwrap();
        let state = read_ralph(tmp.path(), "iter-test").unwrap().unwrap();
        assert_eq!(state.iteration, 3);
    }

    #[tokio::test]
    async fn test_check_at_max_iterations_deletes_and_allows() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set with max_iterations = 2
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("max-test"));
        set_args.insert("instruction".to_string(), serde_json::json!("Do stuff"));
        set_args.insert("max_iterations".to_string(), serde_json::json!(2));
        tool.execute(set_args, &ctx).await.unwrap();

        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("max-test"));

        // Check 1: iteration 1 of 2 — should block
        let r1 = tool.execute(check_args.clone(), &ctx).await.unwrap();
        let j1: serde_json::Value = serde_json::from_str(
            r1.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap(),
        ).unwrap();
        assert_eq!(j1["decision"], "block");

        // Check 2: iteration 2 of 2 — should block (still within limit)
        let r2 = tool.execute(check_args.clone(), &ctx).await.unwrap();
        let j2: serde_json::Value = serde_json::from_str(
            r2.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap(),
        ).unwrap();
        assert_eq!(j2["decision"], "block");

        // Check 3: iteration 3 > max 2 — should allow and delete file
        let r3 = tool.execute(check_args.clone(), &ctx).await.unwrap();
        let j3: serde_json::Value = serde_json::from_str(
            r3.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap(),
        ).unwrap();
        assert_eq!(j3["decision"], "allow");
        assert!(j3["reason"].as_str().unwrap().contains("Max iterations reached"));

        // File should be gone
        assert!(read_ralph(tmp.path(), "max-test").unwrap().is_none());
    }

    #[tokio::test]
    async fn test_check_output_matches_stop_hook_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set instruction
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("session_id".to_string(), serde_json::json!("hook-test"));
        set_args.insert("instruction".to_string(), serde_json::json!("Implement cards"));
        tool.execute(set_args, &ctx).await.unwrap();

        // Check — block response
        let mut check_args = serde_json::Map::new();
        check_args.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args.insert("session_id".to_string(), serde_json::json!("hook-test"));
        let result = tool.execute(check_args, &ctx).await.unwrap();
        let content = result.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap();
        let json: serde_json::Value = serde_json::from_str(content).unwrap();

        // Must have "decision" and "reason" per Claude Code Stop hook spec
        assert!(json.get("decision").is_some(), "Must have 'decision' field");
        assert!(json.get("reason").is_some(), "Must have 'reason' field");
        assert_eq!(json["decision"], "block");
        assert!(json["reason"].as_str().unwrap().len() > 0, "Reason must be non-empty");

        // Allow response also needs valid schema
        let tmp2 = tempfile::tempdir().unwrap();
        let ctx2 = make_context(&tmp2).await;
        let mut check_args2 = serde_json::Map::new();
        check_args2.insert("op".to_string(), serde_json::json!("check ralph"));
        check_args2.insert("session_id".to_string(), serde_json::json!("no-session"));
        let result2 = tool.execute(check_args2, &ctx2).await.unwrap();
        let content2 = result2.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap();
        let json2: serde_json::Value = serde_json::from_str(content2).unwrap();
        assert_eq!(json2["decision"], "allow");
    }

    // --- Session ID defaulting tests ---

    #[tokio::test]
    async fn test_set_ralph_defaults_to_context_session_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set without explicit session_id — should use context.session_id
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("set ralph"));
        args.insert("instruction".to_string(), serde_json::json!("Auto session"));
        tool.execute(args, &ctx).await.unwrap();

        // Verify file was created using context's session_id
        let state = read_ralph(tmp.path(), &ctx.session_id).unwrap();
        assert!(state.is_some(), "State should exist under context session_id");
        assert_eq!(state.unwrap().instruction, "Auto session");
    }

    #[tokio::test]
    async fn test_get_ralph_defaults_to_context_session_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set using context session_id
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("instruction".to_string(), serde_json::json!("Test get"));
        tool.execute(set_args, &ctx).await.unwrap();

        // Get without explicit session_id
        let mut get_args = serde_json::Map::new();
        get_args.insert("op".to_string(), serde_json::json!("get ralph"));
        let result = tool.execute(get_args, &ctx).await.unwrap();
        let content = result.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap();
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["active"], true);
        assert_eq!(json["instruction"], "Test get");
    }

    #[tokio::test]
    async fn test_clear_ralph_defaults_to_context_session_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // Set using context session_id
        let mut set_args = serde_json::Map::new();
        set_args.insert("op".to_string(), serde_json::json!("set ralph"));
        set_args.insert("instruction".to_string(), serde_json::json!("Test clear"));
        tool.execute(set_args, &ctx).await.unwrap();

        // Clear without explicit session_id
        let mut clear_args = serde_json::Map::new();
        clear_args.insert("op".to_string(), serde_json::json!("clear ralph"));
        let result = tool.execute(clear_args, &ctx).await.unwrap();
        let content = result.content.first().and_then(|c| c.as_text()).map(|t| t.text.as_str()).unwrap();
        let json: serde_json::Value = serde_json::from_str(content).unwrap();
        assert_eq!(json["cleared"], true);

        // Verify file is gone
        assert!(read_ralph(tmp.path(), &ctx.session_id).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_check_ralph_requires_explicit_session_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_context(&tmp).await;
        let tool = RalphTool::new();

        // check ralph without session_id should fail
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("check ralph"));
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err(), "check ralph should require explicit session_id");
    }
}
