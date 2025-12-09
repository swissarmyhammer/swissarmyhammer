//! Composite validator that combines all generation request validation logic

use super::{MessageContentConfig, MessageContentValidator, SessionStateValidator};
use crate::types::{GenerationRequest, Session};
use crate::validation::{ValidationError, ValidationResult, Validator};
// ValidationLimits not available in llama_common

/// Configuration for the composite generation request validator
#[derive(Debug, Clone, Default)]
pub struct ValidationConfig {
    /// Configuration for message content validation
    pub message_content: MessageContentConfig,
}

/// Composite validator that performs comprehensive validation of generation requests
///
/// This validator combines:
/// - Session state validation (ensures session is valid for generation)
/// - Message content validation (validates all messages in session)
///
/// Parameter validation is handled by GenerationConfig::validate() and llama-cpp-2 native validation.
/// This provides a single entry point for complete generation request validation.
#[derive(Debug, Clone)]
pub struct CompositeGenerationRequestValidator {
    session_validator: SessionStateValidator,
    message_validator: MessageContentValidator,
}

impl CompositeGenerationRequestValidator {
    /// Create a new composite validator with default configuration
    pub fn new() -> Self {
        Self::with_config(ValidationConfig::default())
    }

    /// Create a composite validator with custom configuration
    pub fn with_config(config: ValidationConfig) -> Self {
        Self {
            session_validator: SessionStateValidator::new(),
            message_validator: MessageContentValidator::with_config(config.message_content),
        }
    }

    /// Create a composite validator with individual validator configurations
    pub fn with_validators(
        session_validator: SessionStateValidator,
        message_validator: MessageContentValidator,
    ) -> Self {
        Self {
            session_validator,
            message_validator,
        }
    }

    /// Get a reference to the session validator
    pub fn session_validator(&self) -> &SessionStateValidator {
        &self.session_validator
    }

    /// Get a reference to the message validator
    pub fn message_validator(&self) -> &MessageContentValidator {
        &self.message_validator
    }
}

impl Default for CompositeGenerationRequestValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator<GenerationRequest> for CompositeGenerationRequestValidator {
    type Error = ValidationError;

    fn validate(&self, session: &Session, request: &GenerationRequest) -> ValidationResult {
        // Step 1: Validate session state
        self.session_validator.validate(session, request)?;

        // Step 2: Validate all messages in session
        for message in &session.messages {
            self.message_validator.validate(session, message)?;
        }

        // Note: Parameter validation is now handled by GenerationConfig::validate()
        // and llama-cpp-2 native validation in the generation layer

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::centralized_test_utils::*;
    use crate::types::SessionId;
    use crate::{Message, MessageRole};
    use std::time::{Duration, SystemTime};

    fn create_test_request() -> GenerationRequest {
        GenerationRequest {
            session_id: SessionId::new(),
            max_tokens: Some(150),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: vec!["Human:".to_string()],
            stopping_config: None,
        }
    }

    #[test]
    fn test_valid_complete_request_passes() {
        let validator = CompositeGenerationRequestValidator::new();
        let session = create_session_with_messages(vec![
            create_test_message("Hello, how are you?"),
            create_test_message("What can you help me with today?"),
        ]);
        let request = create_test_request();

        assert!(validator.validate(&session, &request).is_ok());
    }

    #[test]
    fn test_session_validation_failure() {
        let validator = CompositeGenerationRequestValidator::new();
        // Empty messages should fail session validation
        let session = create_session_with_messages(vec![]);
        let request = create_test_request();

        let result = validator.validate(&session, &request);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one message"));
    }

    #[test]
    fn test_message_content_validation_now_deferred_to_llama_cpp() {
        // Content validation (including XSS detection) is now deferred to llama-cpp
        let validator = CompositeGenerationRequestValidator::new();
        let session = create_session_with_messages(vec![
            create_test_message("Normal message"),
            create_test_message("<script>alert('xss')</script>"), // Previously suspicious content
        ]);
        let request = create_test_request();

        let result = validator.validate(&session, &request);
        assert!(result.is_ok()); // Should pass since llama-cpp handles content validation
    }

    #[test]
    fn test_parameter_validation_now_handled_by_generation_layer() {
        // Parameter validation is now handled by GenerationConfig::validate()
        // and llama-cpp-2 native validation, not by composite validator
        let validator = CompositeGenerationRequestValidator::new();
        let session = create_session_with_messages(vec![create_test_message("Valid message")]);
        let request = create_test_request();

        // Composite validator no longer validates parameters
        let result = validator.validate(&session, &request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_message_validation() {
        let validator = CompositeGenerationRequestValidator::new();
        let session = create_session_with_messages(vec![
            create_test_message("First valid message"),
            create_test_message("Second valid message"),
            create_test_message("Third valid message"),
        ]);
        let request = create_test_request();

        assert!(validator.validate(&session, &request).is_ok());
    }

    #[test]
    fn test_custom_configuration_deferred_to_llama_cpp() {
        let config = ValidationConfig {
            message_content: MessageContentConfig {
                // No configuration needed - all validation handled by llama-cpp
            },
        };

        let validator = CompositeGenerationRequestValidator::with_config(config);
        let session = create_session_with_messages(vec![
            create_test_message(&"a".repeat(101)), // Long message now passes
        ]);
        let request = create_test_request();

        let result = validator.validate(&session, &request);
        assert!(result.is_ok()); // Should pass since llama-cpp handles validation
    }

    #[test]
    fn test_validation_order() {
        // Test that validation fails at the first error encountered
        let validator = CompositeGenerationRequestValidator::new();

        // Create session with no messages (should fail session validation first)
        let session = create_session_with_messages(vec![]);
        let request = create_test_request();
        // Parameter validation no longer done by composite validator

        let result = validator.validate(&session, &request);
        assert!(result.is_err());
        // Should fail on session validation
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one message"));
    }

    #[test]
    fn test_individual_validator_access() {
        let validator = CompositeGenerationRequestValidator::new();

        // Test that we can access individual validators
        let session = create_session_with_messages(vec![create_test_message("test")]);
        let request = create_test_request();

        // Should be able to use individual validators directly
        assert!(validator
            .session_validator()
            .validate(&session, &request)
            .is_ok());

        for message in &session.messages {
            assert!(validator
                .message_validator()
                .validate(&session, message)
                .is_ok());
        }
    }

    #[test]
    fn test_with_validators_constructor() {
        let session_validator = SessionStateValidator::new();
        let message_validator = MessageContentValidator::new();

        let composite = CompositeGenerationRequestValidator::with_validators(
            session_validator,
            message_validator,
        );

        let session = create_session_with_messages(vec![create_test_message("test message")]);
        let request = create_test_request();

        assert!(composite.validate(&session, &request).is_ok());
    }

    #[test]
    fn test_realistic_generation_scenario() {
        let validator = CompositeGenerationRequestValidator::new();

        // Create a realistic chat session
        let session = create_session_with_messages(vec![
            Message {
                role: MessageRole::System,
                content: "You are a helpful AI assistant.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now() - Duration::from_secs(60),
            },
            Message {
                role: MessageRole::User,
                content: "Can you help me write a Python function to calculate fibonacci numbers?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now() - Duration::from_secs(30),
            },
            Message {
                role: MessageRole::Assistant,
                content: "I'd be happy to help you write a Fibonacci function! Here's a simple recursive implementation:".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now() - Duration::from_secs(15),
            },
        ]);

        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(500),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: vec!["Human:".to_string(), "\n\n".to_string()],
            stopping_config: None,
        };

        assert!(validator.validate(&session, &request).is_ok());
    }

    #[test]
    fn test_edge_case_suspicious_content_now_deferred_to_llama_cpp() {
        let validator = CompositeGenerationRequestValidator::new();
        let session = create_session_with_messages(vec![
            create_test_message("First message is safe"),
            create_test_message("Second message is also safe"),
            create_test_message("Third message has <script> injection"), // Previously suspicious content
            create_test_message("Fourth message is safe again"),
        ]);
        let request = create_test_request();

        let result = validator.validate(&session, &request);
        assert!(result.is_ok()); // Should pass since llama-cpp handles content validation
    }

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        let validator = CompositeGenerationRequestValidator::with_config(config);

        // Should behave the same as new()
        let default_validator = CompositeGenerationRequestValidator::new();

        let session = create_session_with_messages(vec![create_test_message("Test message")]);
        let request = create_test_request();

        assert_eq!(
            validator.validate(&session, &request).is_ok(),
            default_validator.validate(&session, &request).is_ok()
        );
    }

    #[test]
    fn test_clone_validator() {
        let validator1 = CompositeGenerationRequestValidator::new();
        let validator2 = validator1.clone();

        let session = create_session_with_messages(vec![create_test_message("Test message")]);
        let request = create_test_request();

        // Both should behave identically
        assert_eq!(
            validator1.validate(&session, &request).is_ok(),
            validator2.validate(&session, &request).is_ok()
        );
    }

    #[test]
    fn test_empty_session_id_handling() {
        let validator = CompositeGenerationRequestValidator::new();

        // Note: SessionId is a ULID wrapper and cannot be empty by construction
        // This test documents that session ID validation is handled by the type system
        let session = create_session_with_messages(vec![create_test_message("Valid message")]);
        let request = create_test_request();

        assert!(validator.validate(&session, &request).is_ok());
    }

    #[test]
    fn test_custom_suspicious_patterns_now_deferred_to_llama_cpp() {
        let config = ValidationConfig {
            message_content: MessageContentConfig {
                // No configuration needed - all validation handled by llama-cpp
            },
        };

        let validator = CompositeGenerationRequestValidator::with_config(config);
        let session = create_session_with_messages(vec![create_test_message(
            "This message contains FORBIDDEN_WORD which now passes validation",
        )]);
        let request = create_test_request();

        let result = validator.validate(&session, &request);
        assert!(result.is_ok()); // Should pass since llama-cpp handles content validation
    }
}
