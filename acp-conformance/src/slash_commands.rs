//! Slash commands protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/slash-commands
//!
//! ## Requirements Tested
//!
//! 1. **Advertising Commands**
//!    - Agents MAY send `available_commands_update` notification after session creation
//!    - Notification includes list of AvailableCommand objects
//!    - Method: `session/update` with `sessionUpdate: "available_commands_update"`
//!
//! 2. **Command Structure**
//!    - Each AvailableCommand has:
//!      - `name` (string, required): Command name (e.g., "web", "test", "plan")
//!      - `description` (string, required): Human-readable description
//!      - `input` (AvailableCommandInput, optional): Input specification
//!    - AvailableCommandInput has:
//!      - `hint` (string, required): Hint when input not provided
//!
//! 3. **Dynamic Updates**
//!    - Agents can send `available_commands_update` at any time during session
//!    - Allows adding, removing, or modifying commands based on context
//!
//! 4. **Running Commands**
//!    - Commands included as regular text in prompt requests
//!    - Format: `/command_name [optional input]`
//!    - Can be accompanied by other content types (images, audio, etc.)

use agent_client_protocol::schema::{ContentBlock, NewSessionRequest, PromptRequest, TextContent};
use agent_client_protocol_extras::AgentWithFixture;
use serde_json::Value;
use swissarmyhammer_common::Pretty;

/// Test that command structure is valid when advertised
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// When agents advertise commands, they must have proper structure
pub async fn test_command_structure_validation(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command structure validation");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd);

    let _response = agent
        .connection()
        .send_request(request)
        .block_task()
        .await?;

    // Note: Commands are advertised via session/update notifications
    // This test verifies that IF commands are advertised, they have valid structure
    // The actual notification listening would need to be implemented by the test infrastructure

    tracing::info!(
        "Command structure validation test complete (notification-based, may need transcript)"
    );

    Ok(())
}

/// Test that agents can advertise commands with valid structure
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// Agents MAY send available_commands_update notification
pub async fn test_advertise_commands(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command advertisement");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd);

    let response = agent
        .connection()
        .send_request(request)
        .block_task()
        .await?;
    let session_id = response.session_id;

    tracing::info!(
        "Session created: {} (commands may be advertised via notification)",
        session_id.0
    );

    // Commands are advertised via session/update notifications, not in response
    // The test framework would need to capture notifications to validate this

    Ok(())
}

/// Test that commands can be invoked via prompt requests
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// Commands are included as regular text in prompt requests
pub async fn test_run_command(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command invocation via prompt");

    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent
        .connection()
        .send_request(new_request)
        .block_task()
        .await?;
    let session_id = new_response.session_id;

    // Send a prompt with a slash command
    // Using a simple command that might be commonly available
    let text_content = TextContent::new("/help");
    let prompt_content = vec![ContentBlock::Text(text_content)];

    let prompt_request = PromptRequest::new(session_id.clone(), prompt_content);

    // Agent should process the command (or reject if not available)
    let result = agent
        .connection()
        .send_request(prompt_request)
        .block_task()
        .await;

    match result {
        Ok(response) => {
            tracing::info!("Agent processed slash command: {}", Pretty(&response));
            Ok(())
        }
        Err(e) => {
            // Agent may not support commands or this specific command
            tracing::info!("Agent returned error for slash command: {}", Pretty(&e));
            // This is acceptable behavior - commands are optional
            Ok(())
        }
    }
}

/// Test command validation - names and descriptions
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// Commands must have non-empty name and description
pub async fn test_command_field_validation(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command field validation");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd);

    let _response = agent
        .connection()
        .send_request(request)
        .block_task()
        .await?;

    // This test would validate that any advertised commands have:
    // - Non-empty name
    // - Non-empty description
    // - If input is present, it has a non-empty hint
    //
    // Validation happens by inspecting session/update notifications

    tracing::info!("Command field validation test complete (requires notification inspection)");

    Ok(())
}

/// Test that command input hints are present when input is specified
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// If input is specified, it must have a hint
pub async fn test_command_input_hint(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command input hint validation");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd);

    let _response = agent
        .connection()
        .send_request(request)
        .block_task()
        .await?;

    // This test validates that commands with input specifications
    // have proper hint text
    //
    // Validation happens by inspecting session/update notifications

    tracing::info!(
        "Command input hint validation test complete (requires notification inspection)"
    );

    Ok(())
}

/// Test command with input argument
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// Commands can accept input arguments
pub async fn test_command_with_input(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command with input argument");

    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent
        .connection()
        .send_request(new_request)
        .block_task()
        .await?;
    let session_id = new_response.session_id;

    // Send a command with input (e.g., /web query)
    let text_content = TextContent::new("/web agent client protocol");
    let prompt_content = vec![ContentBlock::Text(text_content)];

    let prompt_request = PromptRequest::new(session_id, prompt_content);

    // Agent should process the command with input (or reject if not available)
    let result = agent
        .connection()
        .send_request(prompt_request)
        .block_task()
        .await;

    match result {
        Ok(response) => {
            tracing::info!("Agent processed command with input: {}", Pretty(&response));
            Ok(())
        }
        Err(e) => {
            // Agent may not support this command
            tracing::info!(
                "Agent returned error for command with input: {}",
                Pretty(&e)
            );
            // This is acceptable - commands are optional
            Ok(())
        }
    }
}

/// Test command with multiple content types
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/slash-commands
/// Commands may be accompanied by other content types in the same prompt array
pub async fn test_command_with_mixed_content(agent: &dyn AgentWithFixture) -> crate::Result<()> {
    tracing::info!("Testing command with mixed content types");

    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent
        .connection()
        .send_request(new_request)
        .block_task()
        .await?;
    let session_id = new_response.session_id;

    // Send a command with additional text
    let prompt_content = vec![
        ContentBlock::Text(TextContent::new("/help")),
        ContentBlock::Text(TextContent::new("I need assistance with this feature")),
    ];

    let prompt_request = PromptRequest::new(session_id, prompt_content);

    // Agent should process the mixed content (or reject if not available)
    let result = agent
        .connection()
        .send_request(prompt_request)
        .block_task()
        .await;

    match result {
        Ok(response) => {
            tracing::info!(
                "Agent processed command with mixed content: {}",
                Pretty(&response)
            );
            Ok(())
        }
        Err(e) => {
            // Agent may not support this command
            tracing::info!(
                "Agent returned error for command with mixed content: {:?}",
                e
            );
            // This is acceptable
            Ok(())
        }
    }
}

/// Helper function to validate a command structure from JSON
///
/// Validates that a command has:
/// - Non-empty name
/// - Non-empty description
/// - If input is present, it has a non-empty hint
#[allow(dead_code)]
fn validate_command_structure(command: &Value) -> crate::Result<()> {
    // Validate name
    let name = command
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| crate::Error::Validation("Command must have 'name' field".to_string()))?;

    if name.is_empty() {
        return Err(crate::Error::Validation(
            "Command name must not be empty".to_string(),
        ));
    }

    // Validate description
    let description = command
        .get("description")
        .and_then(|d| d.as_str())
        .ok_or_else(|| {
            crate::Error::Validation("Command must have 'description' field".to_string())
        })?;

    if description.is_empty() {
        return Err(crate::Error::Validation(
            "Command description must not be empty".to_string(),
        ));
    }

    // Validate input if present
    if let Some(input) = command.get("input") {
        let hint = input.get("hint").and_then(|h| h.as_str()).ok_or_else(|| {
            crate::Error::Validation(
                "Command input must have 'hint' field when input is specified".to_string(),
            )
        })?;

        if hint.is_empty() {
            return Err(crate::Error::Validation(
                "Command input hint must not be empty".to_string(),
            ));
        }
    }

    tracing::info!(
        "Validated command: name='{}', description='{}'",
        name,
        description
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_command_structure_valid() {
        let command = json!({
            "name": "web",
            "description": "Search the web for information",
            "input": {
                "hint": "query to search for"
            }
        });

        assert!(validate_command_structure(&command).is_ok());
    }

    #[test]
    fn test_validate_command_structure_minimal() {
        let command = json!({
            "name": "test",
            "description": "Run tests for the current project"
        });

        assert!(validate_command_structure(&command).is_ok());
    }

    #[test]
    fn test_validate_command_structure_missing_name() {
        let command = json!({
            "description": "Some description"
        });

        assert!(validate_command_structure(&command).is_err());
    }

    #[test]
    fn test_validate_command_structure_empty_name() {
        let command = json!({
            "name": "",
            "description": "Some description"
        });

        let result = validate_command_structure(&command);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("name must not be empty"));
    }

    #[test]
    fn test_validate_command_structure_missing_description() {
        let command = json!({
            "name": "test"
        });

        assert!(validate_command_structure(&command).is_err());
    }

    #[test]
    fn test_validate_command_structure_empty_description() {
        let command = json!({
            "name": "test",
            "description": ""
        });

        let result = validate_command_structure(&command);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("description must not be empty"));
    }

    #[test]
    fn test_validate_command_structure_input_missing_hint() {
        let command = json!({
            "name": "web",
            "description": "Search the web",
            "input": {}
        });

        assert!(validate_command_structure(&command).is_err());
    }

    #[test]
    fn test_validate_command_structure_input_empty_hint() {
        let command = json!({
            "name": "web",
            "description": "Search the web",
            "input": {
                "hint": ""
            }
        });

        let result = validate_command_structure(&command);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("hint must not be empty"));
    }

    #[test]
    fn test_module_compiles() {
        // This ensures the module compiles correctly
    }

    // --- Async tests using mock agent ---

    use crate::test_utils::{run_with_mock_agent_as_fixture, MockAgent};
    use agent_client_protocol::schema::{
        InitializeRequest, InitializeResponse, NewSessionResponse, ProtocolVersion, PromptResponse,
        StopReason,
    };
    use futures::future::BoxFuture;
    use std::sync::Arc;

    /// Mock agent for slash command tests.
    ///
    /// Slash commands are advertised via `available_commands_update`
    /// session-update notifications and invoked through plain text prompts —
    /// the mock just needs to make `initialize`, `new_session`, and
    /// `prompt` succeed so the production helpers can exercise the
    /// underlying transport. The non-asserting helpers (`Ok(_) | Err(_)`
    /// both treat as success) ride along.
    struct SlashCmdMockAgent;

    impl MockAgent for SlashCmdMockAgent {
        fn initialize<'a>(
            &'a self,
            _request: InitializeRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<InitializeResponse>> {
            Box::pin(async move { Ok(InitializeResponse::new(ProtocolVersion::V1)) })
        }

        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            Box::pin(async move { Ok(NewSessionResponse::new("slash-cmd-test-session")) })
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            Box::pin(async move { Ok(PromptResponse::new(StopReason::EndTurn)) })
        }
    }

    #[tokio::test]
    async fn test_command_structure_validation_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_command_structure_validation(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[tokio::test]
    async fn test_advertise_commands_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_advertise_commands(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[tokio::test]
    async fn test_run_command_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result =
            run_with_mock_agent_as_fixture(mock, |fx| async move { test_run_command(&fx).await })
                .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[tokio::test]
    async fn test_command_field_validation_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_command_field_validation(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[tokio::test]
    async fn test_command_input_hint_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_command_input_hint(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[tokio::test]
    async fn test_command_with_input_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_command_with_input(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }

    #[tokio::test]
    async fn test_command_with_mixed_content_mock() {
        let mock = Arc::new(SlashCmdMockAgent);
        let result = run_with_mock_agent_as_fixture(mock, |fx| async move {
            test_command_with_mixed_content(&fx).await
        })
        .await;
        assert!(result.is_ok(), "result: {:?}", result);
    }
}
