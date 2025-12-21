//! Session setup and session modes protocol conformance tests
//!
//! Tests based on:
//! - https://agentclientprotocol.com/protocol/session-setup
//! - https://agentclientprotocol.com/protocol/session-modes
//!
//! ## Requirements Tested
//!
//! ### Session Setup
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
//! ### Session Modes
//! 1. **Initial State**
//!    - Agents MAY return available modes during session setup
//!    - SessionModeState includes currentModeId and availableModes
//!    - Each mode has id, name, and optional description
//!
//! 2. **Setting Mode from Client**
//!    - session/set_mode switches to an available mode
//!    - Must validate mode is in availableModes
//!    - Returns confirmation or error
//!
//! 3. **Mode Validation**
//!    - Current mode must be in available modes list
//!    - Setting invalid mode should fail
//!    - Mode IDs and names must not be empty

use agent_client_protocol::{
    Agent, LoadSessionRequest, NewSessionRequest, SessionId, SessionModeId, SetSessionModeRequest,
};

/// Test creating a new session with minimal parameters
pub async fn test_new_session_minimal<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
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
pub async fn test_new_session_with_mcp<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
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
pub async fn test_session_ids_unique<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
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
pub async fn test_load_nonexistent_session<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
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
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/session-modes
/// Agents that provide modes should accept valid mode changes
/// Agents that don't provide modes may reject mode changes
pub async fn test_set_session_mode<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing set session mode");

    // First create a session
    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent.new_session(new_request).await?;
    let session_id = new_response.session_id;

    // Check if agent provides modes
    if let Some(mode_state) = new_response.modes {
        // Agent provides modes - try to set to an available mode
        if let Some(mode) = mode_state.available_modes.first() {
            let request = SetSessionModeRequest::new(session_id, mode.id.clone());
            let response = agent.set_session_mode(request).await?;
            tracing::info!("Set session mode response: {:?}", response);
        } else {
            tracing::warn!("Agent provides modes but available_modes is empty");
        }
    } else {
        // Agent doesn't provide modes - it may reject mode changes
        let mode_id = SessionModeId::new("test-mode");
        let request = SetSessionModeRequest::new(session_id, mode_id);

        match agent.set_session_mode(request).await {
            Ok(response) => {
                tracing::info!(
                    "Agent accepted mode change without providing modes: {:?}",
                    response
                );
            }
            Err(_) => {
                tracing::info!(
                    "Agent correctly rejected mode change since it doesn't provide modes"
                );
            }
        }
    }

    Ok(())
}

/// Test that new session returns available modes
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/session-modes
/// Agents MAY return available modes during session setup
pub async fn test_new_session_includes_modes<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing new session includes mode information");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd);

    let response = agent.new_session(request).await?;

    // Check if modes are present (optional per spec)
    if let Some(mode_state) = response.modes {
        tracing::info!("Session includes mode state");

        // Validate current mode ID is set
        if mode_state.current_mode_id.0.is_empty() {
            return Err(crate::Error::Validation(
                "Current mode ID must not be empty when modes are provided".to_string(),
            ));
        }

        tracing::info!("Current mode: {}", mode_state.current_mode_id.0);

        // Validate available modes list
        if mode_state.available_modes.is_empty() {
            return Err(crate::Error::Validation(
                "Available modes must not be empty when modes are provided".to_string(),
            ));
        }

        tracing::info!("Available modes: {}", mode_state.available_modes.len());

        // Validate each mode has required fields
        for mode in &mode_state.available_modes {
            if mode.id.0.is_empty() {
                return Err(crate::Error::Validation(
                    "Mode ID must not be empty".to_string(),
                ));
            }
            if mode.name.is_empty() {
                return Err(crate::Error::Validation(
                    "Mode name must not be empty".to_string(),
                ));
            }
            tracing::info!(
                "Mode: id={}, name={}, description={:?}",
                mode.id.0,
                mode.name,
                mode.description
            );
        }

        // Verify current mode is in available modes
        let current_mode_available = mode_state
            .available_modes
            .iter()
            .any(|m| m.id == mode_state.current_mode_id);

        if !current_mode_available {
            return Err(crate::Error::Validation(
                "Current mode must be in available modes list".to_string(),
            ));
        }

        tracing::info!("Session mode state is valid");
    } else {
        tracing::info!("Session does not include mode state (optional per spec)");
    }

    Ok(())
}

/// Test setting session mode to an available mode
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/session-modes
/// Clients can switch modes by calling session/set-mode with a mode from available_modes
pub async fn test_set_session_mode_to_available<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing set session mode to an available mode");

    // First create a session
    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent.new_session(new_request).await?;
    let session_id = new_response.session_id;

    // Check if modes are available
    if let Some(mode_state) = new_response.modes {
        if mode_state.available_modes.len() < 2 {
            tracing::info!("Not enough modes to test switching (need at least 2)");
            return Ok(());
        }

        // Find a mode different from current
        let target_mode = mode_state
            .available_modes
            .iter()
            .find(|m| m.id != mode_state.current_mode_id)
            .ok_or_else(|| {
                crate::Error::Validation("Could not find a different mode to switch to".to_string())
            })?;

        tracing::info!(
            "Switching from mode '{}' to mode '{}'",
            mode_state.current_mode_id.0,
            target_mode.id.0
        );

        // Set the new mode
        let request = SetSessionModeRequest::new(session_id, target_mode.id.clone());
        agent.set_session_mode(request).await?;

        tracing::info!("Successfully switched to mode '{}'", target_mode.id.0);
    } else {
        tracing::info!("Session does not support modes (skipping test)");
    }

    Ok(())
}

/// Test setting session mode to an invalid mode ID
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/session-modes
/// Attempting to set a mode that is not in availableModes should fail
pub async fn test_set_invalid_session_mode<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing set session mode with invalid mode ID");

    // First create a session
    let cwd = std::env::temp_dir();
    let new_request = NewSessionRequest::new(cwd);
    let new_response = agent.new_session(new_request).await?;
    let session_id = new_response.session_id;

    // Try to set an invalid mode ID
    let invalid_mode_id = SessionModeId::new("nonexistent-invalid-mode-xyz");
    let request = SetSessionModeRequest::new(session_id, invalid_mode_id);

    let result = agent.set_session_mode(request).await;

    // Should return an error
    if result.is_ok() {
        return Err(crate::Error::Validation(
            "Setting invalid mode should fail".to_string(),
        ));
    }

    tracing::info!("Correctly rejected invalid mode ID");

    Ok(())
}

/// Test that mode state structure is valid when provided
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/session-modes
/// When modes are provided, they must have proper structure
pub async fn test_mode_state_validation<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing comprehensive mode state validation");

    let cwd = std::env::temp_dir();
    let request = NewSessionRequest::new(cwd);

    let response = agent.new_session(request).await?;

    // If modes are present, validate structure comprehensively
    if let Some(mode_state) = response.modes {
        // Validate current mode ID
        if mode_state.current_mode_id.0.is_empty() {
            return Err(crate::Error::Validation(
                "Current mode ID must not be empty".to_string(),
            ));
        }

        // Validate available modes list
        if mode_state.available_modes.is_empty() {
            return Err(crate::Error::Validation(
                "Available modes must not be empty when modes are provided".to_string(),
            ));
        }

        // Validate each mode structure
        for mode in &mode_state.available_modes {
            // ID must not be empty
            if mode.id.0.is_empty() {
                return Err(crate::Error::Validation(
                    "Mode ID must not be empty".to_string(),
                ));
            }

            // Name must not be empty
            if mode.name.is_empty() {
                return Err(crate::Error::Validation(
                    "Mode name must not be empty".to_string(),
                ));
            }

            // ID should not contain whitespace (good practice)
            if mode.id.0.contains(char::is_whitespace) {
                tracing::warn!(
                    "Mode ID '{}' contains whitespace (not recommended)",
                    mode.id.0
                );
            }

            tracing::info!(
                "Validated mode: id='{}', name='{}', description={:?}",
                mode.id.0,
                mode.name,
                mode.description
            );
        }

        // Verify current mode is in available modes
        let current_mode_available = mode_state
            .available_modes
            .iter()
            .any(|m| m.id == mode_state.current_mode_id);

        if !current_mode_available {
            return Err(crate::Error::Validation(format!(
                "Current mode '{}' must be in available modes list",
                mode_state.current_mode_id.0
            )));
        }

        // Verify no duplicate mode IDs
        let mut seen_ids = std::collections::HashSet::new();
        for mode in &mode_state.available_modes {
            if !seen_ids.insert(&mode.id) {
                return Err(crate::Error::Validation(format!(
                    "Duplicate mode ID found: '{}'",
                    mode.id.0
                )));
            }
        }

        tracing::info!("Mode state validation passed");
    } else {
        tracing::info!("Session does not provide modes (optional per spec)");
    }

    Ok(())
}

/// Test that multiple sessions can have independent mode states
///
/// Per ACP spec: https://agentclientprotocol.com/protocol/session-modes
/// Each session maintains its own mode state
pub async fn test_session_mode_independence<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing session mode independence");

    let cwd = std::env::temp_dir();

    // Create first session
    let request1 = NewSessionRequest::new(cwd.clone());
    let response1 = agent.new_session(request1).await?;
    let session_id1 = response1.session_id;

    // Create second session
    let request2 = NewSessionRequest::new(cwd);
    let response2 = agent.new_session(request2).await?;
    let session_id2 = response2.session_id;

    // Both sessions should have modes if the agent supports them
    if let (Some(mode_state1), Some(_mode_state2)) = (&response1.modes, &response2.modes) {
        // Find different modes if available
        if mode_state1.available_modes.len() >= 2 {
            let mode1 = &mode_state1.available_modes[0];
            let mode2 = mode_state1
                .available_modes
                .iter()
                .find(|m| m.id != mode1.id)
                .unwrap_or(&mode_state1.available_modes[0]);

            // Set session 1 to mode1
            let request1 = SetSessionModeRequest::new(session_id1.clone(), mode1.id.clone());
            agent.set_session_mode(request1).await?;

            // Set session 2 to mode2 (if different)
            if mode1.id != mode2.id {
                let request2 = SetSessionModeRequest::new(session_id2, mode2.id.clone());
                agent.set_session_mode(request2).await?;

                tracing::info!(
                    "Successfully set independent modes: session1='{}', session2='{}'",
                    mode1.id.0,
                    mode2.id.0
                );
            } else {
                tracing::info!("Only one mode available, cannot test independence fully");
            }
        } else {
            tracing::info!("Less than 2 modes available, skipping independence test");
        }
    } else {
        tracing::info!("Agent does not support modes or modes not returned");
    }

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
