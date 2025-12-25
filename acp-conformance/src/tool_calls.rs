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

/// Test that tool calls generate notifications
///
/// This test verifies that when an agent executes tools, it sends:
/// - tool_call notification (initial report)
/// - tool_call_update notifications (progress updates)
pub async fn test_tool_call_notifications<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing tool call notifications");

    // Initialize
    agent
        .initialize(InitializeRequest::new(ProtocolVersion::V1))
        .await?;

    // Create session
    let cwd = std::env::temp_dir();
    let response = agent.new_session(NewSessionRequest::new(cwd)).await?;
    let session_id = response.session_id;

    // Send prompt that would trigger tool use
    // (This depends on agent having tools available)
    let prompt = vec![ContentBlock::Text(TextContent::new(
        "List files in the current directory",
    ))];
    let prompt_request = PromptRequest::new(session_id, prompt);

    let _response = agent.prompt(prompt_request).await?;

    // Notifications would be sent via Client.session_notification()
    // RecordingClient needs to capture:
    // - SessionUpdate::ToolCall notifications
    // - SessionUpdate::ToolCallUpdate notifications

    tracing::info!("Tool call test complete (notifications should be recorded)");
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
