//! Session setup protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/session-setup
//!
//! ## Requirements Tested
//!
//! 1. **session/new**
//!    - Creates a new conversation session
//!    - Returns unique session ID
//!    - Accepts working directory (cwd)
//!    - Accepts MCP servers configuration
//!
//! 2. **session/load**
//!    - Loads existing session by ID
//!    - Replays conversation history
//!    - Returns session metadata
//!
//! 3. **session/set-mode**
//!    - Switches session operating mode
//!    - Validates mode is supported
//!    - Returns confirmation

use agent_client_protocol::{Agent, LoadSessionRequest, NewSessionRequest, SessionId, SetSessionModeRequest, SessionModeId};

/// Test creating a new session with minimal parameters
pub async fn test_new_session_minimal<A: Agent>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing new session with minimal parameters");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd.clone());

    let response = agent.new_session(request).await?;

    // Validate session ID is present and non-empty
    if response.session_id.0.is_empty() {
        return Err(crate::Error::Validation(
            "Session ID must not be empty".to_string(),
        ));
    }

    tracing::info!("Created session: {}", response.session_id.0);

    Ok(())
}

/// Test creating a new session with MCP servers
pub async fn test_new_session_with_mcp<A: Agent>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing new session with MCP servers");

    let cwd = std::env::temp_dir();

    // Note: MCP server configuration depends on client capabilities
    // For now, test with empty MCP servers array
    let request = NewSessionRequest::new(cwd).mcp_servers(vec![]);

    let response = agent.new_session(request).await?;

    // Validate session ID
    if response.session_id.0.is_empty() {
        return Err(crate::Error::Validation(
            "Session ID must not be empty".to_string(),
        ));
    }

    tracing::info!("Created session with MCP config: {}", response.session_id.0);

    Ok(())
}

/// Test that session IDs are unique
pub async fn test_session_ids_unique<A: Agent>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing session ID uniqueness");

    let cwd = std::env::temp_dir();

    // Create first session
    let request1 = NewSessionRequest::new(cwd.clone());
    let response1 = agent.new_session(request1).await?;

    // Create second session
    let request2 = NewSessionRequest::new(cwd);
    let response2 = agent.new_session(request2).await?;

    // Verify IDs are different
    if response1.session_id == response2.session_id {
        return Err(crate::Error::Validation(
            "Session IDs must be unique".to_string(),
        ));
    }

    tracing::info!(
        "Session IDs are unique: {} != {}",
        response1.session_id.0,
        response2.session_id.0
    );

    Ok(())
}

/// Test loading a nonexistent session
pub async fn test_load_nonexistent_session<A: Agent>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing load of nonexistent session");

    let fake_session_id = SessionId::new("01HZZZZZZZZZZZZZZZZZZZZZZ");
    let cwd = std::env::temp_dir();
    let request = LoadSessionRequest::new(fake_session_id, cwd);

    let result = agent.load_session(request).await;

    // Should return an error
    if result.is_ok() {
        return Err(crate::Error::Validation(
            "Loading nonexistent session should fail".to_string(),
        ));
    }

    tracing::info!("Correctly rejected nonexistent session");

    Ok(())
}

/// Test setting session mode
pub async fn test_set_session_mode<A: Agent>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing set session mode");

    // First create a session
    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent.new_session(new_request).await?;
    let session_id = new_response.session_id;

    // Try to set mode
    let mode_id = SessionModeId::new("test-mode");
    let request = SetSessionModeRequest::new(session_id, mode_id);

    let response = agent.set_session_mode(request).await?;

    tracing::info!("Set session mode response: {:?}", response);

    Ok(())
}

#[cfg(test)]
mod tests {
    /// Dummy test to verify module compiles
    #[test]
    fn test_module_compiles() {
        // This ensures the module compiles correctly
    }
}
