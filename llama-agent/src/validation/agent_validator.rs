//! Agent-specific validation logic for session state and operations

use super::{ValidationError, ValidationResult, Validator};
use crate::types::{Session, SessionError};

/// Validates agent operations and session state before processing
///
/// The `AgentValidator` ensures that sessions are in a valid state for
/// agent operations, particularly generation requests. It performs
/// essential checks that must pass before any agent processing begins.
///
/// # Validations Performed
///
/// - **Message Presence**: Ensures the session has at least one message
/// - **Session ID**: Verifies the session has a valid, non-empty ID
///
/// # Usage
///
/// ```rust
/// use crate::validation::{AgentValidator, Validator};
/// use crate::types::Session;
///
/// let validator = AgentValidator::new();
/// let result = validator.validate(&session, &session);
/// match result {
///     Ok(()) => println!("Session is valid for agent operations"),
///     Err(e) => println!("Validation failed: {}", e),
/// }
/// ```
///
/// # Thread Safety
///
/// This validator is stateless and can be safely used across multiple threads.
pub struct AgentValidator;

impl AgentValidator {
    /// Creates a new agent validator
    ///
    /// # Returns
    ///
    /// A new `AgentValidator` instance ready for use
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator<Session> for AgentValidator {
    type Error = ValidationError;

    fn validate(&self, _context: &Session, target: &Session) -> ValidationResult {
        // Validate session has messages for generation
        if target.messages.is_empty() {
            return Err(ValidationError::invalid_state(
                "Session must have at least one message for generation",
            ));
        }

        // Validate session ID is valid
        if target.id.to_string().is_empty() {
            return Err(ValidationError::invalid_state(
                "Session must have a valid ID",
            ));
        }

        Ok(())
    }
}

/// Convert SessionError to ValidationError for compatibility
impl From<SessionError> for ValidationError {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::NotFound(msg) => {
                ValidationError::invalid_state(format!("Session not found: {}", msg))
            }
            SessionError::InvalidState(msg) => ValidationError::invalid_state(msg),
            SessionError::LimitExceeded => {
                ValidationError::parameter_bounds("Session limit exceeded")
            }
            SessionError::Timeout => ValidationError::invalid_state("Session operation timed out"),
        }
    }
}

impl From<ValidationError> for SessionError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::InvalidState(msg) => SessionError::InvalidState(msg),
            ValidationError::ParameterBounds(msg) => {
                SessionError::InvalidState(format!("Parameter bounds: {}", msg))
            }
            ValidationError::SecurityViolation(msg) => {
                SessionError::InvalidState(format!("Security violation: {}", msg))
            }
            ValidationError::ContentValidation(msg) => {
                SessionError::InvalidState(format!("Content validation: {}", msg))
            }
            ValidationError::SchemaValidation(msg) => {
                SessionError::InvalidState(format!("Schema validation: {}", msg))
            }
            ValidationError::Multiple(errors) => {
                let combined_msg = errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ");
                SessionError::InvalidState(format!("Multiple validation errors: {}", combined_msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessageRole, SessionId};
    use std::time::SystemTime;

    fn create_test_session_with_messages() -> Session {
        Session {
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Test message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: vec![],
            available_tools: vec![],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,
            cached_message_count: 0,
            cached_token_count: 0,
        }
    }

    fn create_empty_session() -> Session {
        let mut session = create_test_session_with_messages();
        session.messages.clear();
        session
    }

    #[test]
    fn test_agent_validator_valid_session() {
        let validator = AgentValidator::new();
        let session = create_test_session_with_messages();

        let result = validator.validate(&session, &session);
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_validator_empty_messages() {
        let validator = AgentValidator::new();
        let context = create_test_session_with_messages();
        let empty_session = create_empty_session();

        let result = validator.validate(&context, &empty_session);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::InvalidState(_)));
        assert!(err.to_string().contains("at least one message"));
    }

    #[test]
    fn test_session_error_conversion() {
        let session_error = SessionError::InvalidState("test error".to_string());
        let validation_error: ValidationError = session_error.into();

        assert!(matches!(validation_error, ValidationError::InvalidState(_)));
        assert!(validation_error.to_string().contains("test error"));
    }

    #[test]
    fn test_session_error_limit_exceeded_conversion() {
        let session_error = SessionError::LimitExceeded;
        let validation_error: ValidationError = session_error.into();

        assert!(matches!(
            validation_error,
            ValidationError::ParameterBounds(_)
        ));
        assert!(validation_error.to_string().contains("limit exceeded"));
    }
}
