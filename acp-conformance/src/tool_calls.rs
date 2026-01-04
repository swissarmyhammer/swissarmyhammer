//! Tool calls protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/tool-calls
//!
//! ## Requirements Tested
//!
//! 1. **Tool Call Reporting**
//!    - Agent sends `tool_call` notification when LLM requests tool execution
//!    - Includes: toolCallId, title, kind, status (pending)
//!    - Method: `session/update` with `sessionUpdate: "tool_call"`
//!
//! 2. **Progress Updates**
//!    - Agent sends `tool_call_update` notifications during execution
//!    - Status transitions: pending → in_progress → completed/failed
//!    - All fields except toolCallId optional in updates
//!
//! 3. **Permission Workflow**
//!    - Agent MAY request permission via `session/request_permission`
//!    - Client responds with outcome (allow/reject)
//!    - If cancelled, client MUST respond with cancelled outcome
//!
//! 4. **Content Types**
//!    - Text content from tool output
//!    - Diffs for file modifications (oldText/newText)
//!    - Terminal output for embedded sessions
//!    - Locations for follow-along tracking

use agent_client_protocol::{
    Agent, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, ProtocolVersion,
    TextContent,
};
use agent_client_protocol_extras::recording::RecordedSession;
use swissarmyhammer_common::Pretty;

/// Statistics from fixture verification
#[derive(Debug, Default, serde::Serialize)]
pub struct ToolCallStats {
    pub tool_calls: usize,
    pub tool_call_updates: usize,
    pub tool_call_completed: usize,
    pub agent_message_chunks: usize,
    pub mcp_progress: usize,
    pub mcp_log: usize,
    pub available_commands_updates: usize,
    pub user_message_chunks: usize,
}

/// Test that tool calls generate notifications
///
/// This test verifies that when an agent executes tools, it sends:
/// - tool_call notification (initial report)
/// - tool_call_update notifications (progress updates)
/// - MCP logging and progress notifications are captured
pub async fn test_tool_call_notifications<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing tool call notifications with TestMcpServer");

    // Initialize
    agent
        .initialize(InitializeRequest::new(ProtocolVersion::V1))
        .await?;

    // Create session - TestMcpServer is already configured in agent factory
    let cwd = std::env::temp_dir();
    let response = agent.new_session(NewSessionRequest::new(cwd)).await?;
    let session_id = response.session_id;

    tracing::info!("Session created with id: {}", session_id);

    // Send prompt that instructs agent to use TestMcpServer tools
    // TestMcpServer provides:
    // - list-files: lists files and sends Progress notifications
    // - create-plan: creates a plan and sends Progress notifications
    // Both tools also send Log notifications
    let prompt = vec![ContentBlock::Text(TextContent::new(
        "Use the mcp__test-mcp-server__list-files tool with path '/tmp' and mcp__test-mcp-server__create-plan tool with goal 'test'",
    ))];
    let prompt_request = PromptRequest::new(session_id, prompt);

    let response = agent.prompt(prompt_request).await?;

    // Verify we got a response
    tracing::info!(
        "Prompt response stop_reason: {}",
        Pretty(&response.stop_reason)
    );

    Ok(())
}

/// Verify tool call notifications in a recorded fixture
///
/// This function reads the fixture and verifies:
/// 1. tool_call notifications were sent
/// 2. tool_call_update notifications with completed status
/// 3. MCP Progress notifications were captured
/// 4. MCP Log notifications were captured
pub fn verify_tool_call_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<ToolCallStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = ToolCallStats::default();

    // Verify we have calls recorded
    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: []"
    );

    for call in &session.calls {
        for notification_json in &call.notifications {
            // Check for ACP session updates
            if let Some(update_val) = notification_json.get("update") {
                if let Some(session_update) =
                    update_val.get("sessionUpdate").and_then(|v| v.as_str())
                {
                    match session_update {
                        "agent_message_chunk" => stats.agent_message_chunks += 1,
                        "user_message_chunk" => stats.user_message_chunks += 1,
                        "tool_call" => {
                            stats.tool_calls += 1;
                            // Verify tool_call has required fields
                            assert!(
                                update_val.get("toolCallId").is_some(),
                                "tool_call must have toolCallId"
                            );
                            assert!(
                                update_val.get("title").is_some(),
                                "tool_call must have title"
                            );
                        }
                        "tool_call_update" => {
                            stats.tool_call_updates += 1;
                            // Check if this is a completed status
                            if let Some(status) = update_val.get("status").and_then(|v| v.as_str())
                            {
                                if status == "completed" {
                                    stats.tool_call_completed += 1;
                                }
                            }
                        }
                        "available_commands_update" => stats.available_commands_updates += 1,
                        _ => {}
                    }
                }
            }

            // Check for MCP Progress notifications (from proxy capture)
            if notification_json.get("Progress").is_some() {
                stats.mcp_progress += 1;
            }

            // Check for MCP Log notifications (from proxy capture)
            if notification_json.get("Log").is_some() {
                stats.mcp_log += 1;
            }
        }
    }

    tracing::info!("{} fixture stats: {}", agent_type, Pretty(&stats));

    // Core assertions - tool calls must generate notifications
    assert!(
        stats.tool_calls > 0,
        "Expected tool_call notifications, got {}. Agent must send tool_call when LLM requests tool execution.",
        stats.tool_calls
    );

    assert!(
        stats.tool_call_updates > 0,
        "Expected tool_call_update notifications, got {}. Agent must send updates during tool execution.",
        stats.tool_call_updates
    );

    assert!(
        stats.tool_call_completed > 0,
        "Expected completed tool_call_update notifications, got {}. Tool execution should complete.",
        stats.tool_call_completed
    );

    // MCP notifications should be captured by proxy
    assert!(
        stats.mcp_progress > 0,
        "Expected MCP Progress notifications, got {}. TestMcpServer sends progress during tool execution.",
        stats.mcp_progress
    );

    assert!(
        stats.mcp_log > 0,
        "Expected MCP Log notifications, got {}. TestMcpServer sends log messages during tool execution.",
        stats.mcp_log
    );

    // Agent should produce output
    assert!(
        stats.agent_message_chunks > 0,
        "Expected agent_message_chunk notifications, got {}. Agent should respond to prompt.",
        stats.agent_message_chunks
    );

    Ok(stats)
}

/// Test that agents send available_commands_update when commands change
pub async fn test_commands_update_notification<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing available_commands_update notification");

    agent
        .initialize(InitializeRequest::new(ProtocolVersion::V1))
        .await?;

    let cwd = std::env::temp_dir();
    let response = agent.new_session(NewSessionRequest::new(cwd)).await?;
    let session_id = response.session_id;

    tracing::info!("Session created with id: {}", session_id);

    // Send a simple prompt to trigger command updates
    let prompt = vec![ContentBlock::Text(TextContent::new("Hello"))];
    let prompt_request = PromptRequest::new(session_id, prompt);
    let _response = agent.prompt(prompt_request).await?;

    Ok(())
}

/// Verify commands update notifications in a recorded fixture
pub fn verify_commands_update_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<ToolCallStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = ToolCallStats::default();

    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: []"
    );

    for call in &session.calls {
        for notification_json in &call.notifications {
            if let Some(update_val) = notification_json.get("update") {
                if let Some(session_update) =
                    update_val.get("sessionUpdate").and_then(|v| v.as_str())
                {
                    if session_update == "available_commands_update" {
                        stats.available_commands_updates += 1;

                        // Verify structure
                        assert!(
                            update_val.get("availableCommands").is_some(),
                            "available_commands_update must have availableCommands array"
                        );
                    }
                }
            }
        }
    }

    tracing::info!("{} commands update stats: {}", agent_type, Pretty(&stats));

    // Agents MAY send available_commands_update - this is optional per spec
    // But if they do, they must be properly structured (checked above)

    Ok(stats)
}
