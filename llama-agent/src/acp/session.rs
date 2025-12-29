//! ACP session state types
//!
//! This module defines session state management for ACP sessions.

use agent_client_protocol::{AvailableCommand, ClientCapabilities, SessionId as AcpSessionId};
use std::time::SystemTime;

use crate::types::ids::SessionId as LlamaSessionId;

use super::error::SessionError;
use super::permissions::PermissionStorage;

/// State for an ACP session
#[derive(Clone)]
pub struct AcpSessionState {
    /// ACP session identifier
    pub session_id: AcpSessionId,

    /// Corresponding llama-agent session ID
    pub llama_session_id: LlamaSessionId,

    /// Current session mode
    pub mode: SessionMode,

    /// Client capabilities from initialize
    pub client_capabilities: ClientCapabilities,

    /// Permission storage
    pub permissions: PermissionStorage,

    /// Available commands (updated dynamically)
    pub available_commands: Vec<AvailableCommand>,

    /// Creation timestamp
    pub created_at: SystemTime,
}

/// Session mode
#[derive(Clone)]
pub enum SessionMode {
    /// Normal code editing mode
    Code,
    /// Planning mode
    Plan,
    /// Testing mode
    Test,
    /// Custom mode
    Custom(String),
}

impl SessionMode {
    /// Parse a mode ID string into a SessionMode
    pub fn parse(mode_id: &str) -> Result<Self, SessionError> {
        match mode_id {
            "code" => Ok(SessionMode::Code),
            "plan" => Ok(SessionMode::Plan),
            "test" => Ok(SessionMode::Test),
            s => Ok(SessionMode::Custom(s.to_string())),
        }
    }
}

impl AcpSessionState {
    /// Create a new ACP session state
    pub fn new(llama_session_id: LlamaSessionId) -> Self {
        Self {
            session_id: AcpSessionId::new(llama_session_id.to_string()),
            llama_session_id,
            mode: SessionMode::Code,
            client_capabilities: ClientCapabilities::default(),
            permissions: PermissionStorage::new(),
            available_commands: Vec::new(),
            created_at: SystemTime::now(),
        }
    }

    /// Create a new ACP session state with client capabilities
    pub fn with_capabilities(
        llama_session_id: LlamaSessionId,
        client_capabilities: ClientCapabilities,
    ) -> Self {
        Self {
            session_id: AcpSessionId::new(llama_session_id.to_string()),
            llama_session_id,
            mode: SessionMode::Code,
            client_capabilities,
            permissions: PermissionStorage::new(),
            available_commands: Vec::new(),
            created_at: SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_new() {
        // Create a new llama session ID
        let llama_id = LlamaSessionId::new();

        // Create session state
        let state = AcpSessionState::new(llama_id);

        // Verify session IDs match
        assert_eq!(state.llama_session_id, llama_id);
        assert_eq!(state.session_id.0.as_ref(), llama_id.to_string().as_str());

        // Verify default values
        assert!(matches!(state.mode, SessionMode::Code));
        assert!(state.available_commands.is_empty());

        // Verify permissions storage is initialized
        // (we can't easily test the internals without exposing them)

        // Verify timestamp is recent (within last second)
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(state.created_at)
            .expect("Created timestamp should be in the past");
        assert!(
            elapsed.as_secs() < 1,
            "Creation timestamp should be very recent"
        );
    }

    #[test]
    fn test_session_state_with_capabilities() {
        // Create a new llama session ID
        let llama_id = LlamaSessionId::new();

        // Create client capabilities with default values
        let client_caps = ClientCapabilities::default();

        // Create session state with capabilities
        let state = AcpSessionState::with_capabilities(llama_id, client_caps.clone());

        // Verify session IDs match
        assert_eq!(state.llama_session_id, llama_id);
        assert_eq!(state.session_id.0.as_ref(), llama_id.to_string().as_str());

        // Verify client capabilities are set (we just check it's not panicking)
        // The actual capabilities depend on the external crate's default implementation

        // Verify default values
        assert!(matches!(state.mode, SessionMode::Code));
        assert!(state.available_commands.is_empty());

        // Verify timestamp is recent
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(state.created_at)
            .expect("Created timestamp should be in the past");
        assert!(
            elapsed.as_secs() < 1,
            "Creation timestamp should be very recent"
        );
    }

    #[test]
    fn test_session_state_clone() {
        // Create a new session state
        let llama_id = LlamaSessionId::new();
        let state1 = AcpSessionState::new(llama_id);

        // Clone it
        let state2 = state1.clone();

        // Verify they have the same values
        assert_eq!(state1.llama_session_id, state2.llama_session_id);
        assert_eq!(state1.session_id.0, state2.session_id.0);
        assert_eq!(state1.created_at, state2.created_at);
    }

    #[test]
    fn test_multiple_session_states_have_unique_ids() {
        // Create multiple session states
        let state1 = AcpSessionState::new(LlamaSessionId::new());
        let state2 = AcpSessionState::new(LlamaSessionId::new());
        let state3 = AcpSessionState::new(LlamaSessionId::new());

        // Verify all session IDs are unique
        assert_ne!(state1.session_id.0, state2.session_id.0);
        assert_ne!(state1.session_id.0, state3.session_id.0);
        assert_ne!(state2.session_id.0, state3.session_id.0);

        assert_ne!(state1.llama_session_id, state2.llama_session_id);
        assert_ne!(state1.llama_session_id, state3.llama_session_id);
        assert_ne!(state2.llama_session_id, state3.llama_session_id);
    }

    #[test]
    fn test_session_mode_parse() {
        // Test standard modes
        assert!(matches!(
            SessionMode::parse("code").unwrap(),
            SessionMode::Code
        ));
        assert!(matches!(
            SessionMode::parse("plan").unwrap(),
            SessionMode::Plan
        ));
        assert!(matches!(
            SessionMode::parse("test").unwrap(),
            SessionMode::Test
        ));

        // Test custom mode
        match SessionMode::parse("custom-mode").unwrap() {
            SessionMode::Custom(s) => assert_eq!(s, "custom-mode"),
            _ => panic!("Expected Custom mode"),
        }

        // Test empty string creates custom mode
        match SessionMode::parse("").unwrap() {
            SessionMode::Custom(s) => assert_eq!(s, ""),
            _ => panic!("Expected Custom mode"),
        }
    }

    #[test]
    fn test_session_mode_clone() {
        // Test cloning each mode variant
        let code = SessionMode::Code;
        let code_clone = code.clone();
        assert!(matches!(code_clone, SessionMode::Code));

        let plan = SessionMode::Plan;
        let plan_clone = plan.clone();
        assert!(matches!(plan_clone, SessionMode::Plan));

        let test = SessionMode::Test;
        let test_clone = test.clone();
        assert!(matches!(test_clone, SessionMode::Test));

        let custom = SessionMode::Custom("my-mode".to_string());
        match custom.clone() {
            SessionMode::Custom(s) => assert_eq!(s, "my-mode"),
            _ => panic!("Expected Custom mode"),
        }
    }

    #[test]
    fn test_session_state_default_mode_is_code() {
        let llama_id = LlamaSessionId::new();
        let state = AcpSessionState::new(llama_id);

        assert!(matches!(state.mode, SessionMode::Code));
    }

    #[test]
    fn test_session_state_default_available_commands_empty() {
        let llama_id = LlamaSessionId::new();
        let state = AcpSessionState::new(llama_id);

        assert!(state.available_commands.is_empty());
    }
}
