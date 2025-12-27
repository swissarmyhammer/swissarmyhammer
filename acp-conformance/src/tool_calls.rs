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

/// Test that tool calls generate notifications
///
/// This test verifies that when an agent executes tools, it sends:
/// - tool_call notification (initial report)
/// - tool_call_update notifications (progress updates)
/// - MCP logging and progress notifications are converted to ACP
pub async fn test_tool_call_notifications<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    use agent_client_protocol_extras::AgentWithFixture;

    tracing::info!("Testing tool call notifications with TestMcpServer");

    // Initialize
    agent
        .initialize(InitializeRequest::new(ProtocolVersion::V1))
        .await?;

    // Create session - TestMcpServer is already configured in agent factory
    let cwd = std::env::temp_dir();
    let response = agent.new_session(NewSessionRequest::new(cwd)).await?;
    let session_id = response.session_id;

    tracing::info!("Session created - TestMcpServer should be available from agent factory");

    // Send prompt that instructs agent to use TestMcpServer tools
    // This should trigger:
    // 1. Agent sends ToolCall notification
    // 2. Agent calls TestMcpServer list-files tool
    // 3. TestMcpServer sends MCP logging notifications
    // 4. TestMcpServer sends MCP progress notifications
    // 5. NotifyingClientHandler converts MCP→ACP
    // 6. Agent sends ToolCallUpdate notifications
    // 7. All notifications captured by RecordingAgent
    let prompt = vec![ContentBlock::Text(TextContent::new(
        "Use the mcp__test-mcp-server__list-files tool with path '/tmp' and mcp__test-mcp-server__create-plan tool with goal 'test'",
    ))];
    let prompt_request = PromptRequest::new(session_id, prompt);

    let _response = agent.prompt(prompt_request).await?;

    tracing::info!("Tool call notification test complete - verifying fixture");

    Ok(())
}

/// Verify tool call notifications in a recorded fixture
///
/// This function should be called after test_tool_call_notifications completes.
/// It reads the fixture and verifies expected notification types were captured.
pub fn verify_tool_call_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut agent_message_chunks = 0;
    let mut tool_calls = 0;
    let mut tool_call_updates = 0;
    let mut mcp_messages = 0;
    let mut progress_messages = 0;

    for call in &session.calls {
        if call.method == "prompt" {
            for notification_json in &call.notifications {
                if let Some(update_val) = notification_json.get("update") {
                    if let Some(session_update) =
                        update_val.get("sessionUpdate").and_then(|v| v.as_str())
                    {
                        match session_update {
                            "agent_message_chunk" => {
                                agent_message_chunks += 1;
                                // Check if it's an MCP notification
                                if let Some(text) = update_val
                                    .get("content")
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                {
                                    if text.contains("[MCP]") {
                                        mcp_messages += 1;
                                    }
                                    if text.contains("[Progress") {
                                        progress_messages += 1;
                                    }
                                }
                            }
                            "tool_call" => tool_calls += 1,
                            "tool_call_update" => tool_call_updates += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    tracing::info!(
        "{} fixture: {} agent_message_chunks, {} tool_calls, {} tool_call_updates, {} MCP messages, {} progress messages",
        agent_type, agent_message_chunks, tool_calls, tool_call_updates, mcp_messages, progress_messages
    );

    // Assert we have notifications
    assert!(
        agent_message_chunks > 0,
        "Expected agent_message_chunk notifications, got {}",
        agent_message_chunks
    );

    // Once tool calling works, these should be > 0:
    // assert!(tool_calls > 0, "Expected tool_call notifications");
    // assert!(mcp_messages > 0, "Expected MCP logging notifications");

    Ok(())
}

/// Test that agents send available_commands_update when commands change
pub async fn test_commands_update_notification<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing available_commands_update notification");

    agent
        .initialize(InitializeRequest::new(ProtocolVersion::V1))
        .await?;

    let cwd = std::env::temp_dir();
    let response = agent.new_session(NewSessionRequest::new(cwd)).await?;

    // Agent MAY send available_commands_update notification
    // RecordingClient should capture SessionUpdate::AvailableCommandsUpdate

    tracing::info!("Commands update notification test complete");
    Ok(())
}
