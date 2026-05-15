//! Validation system for agent requests and data
//!
//! This module provides a trait-based validation system with modular,
//! composable validation logic and clear separation of concerns.

pub mod agent_validator;
pub mod errors;
pub mod generation_request;
pub mod mcp_validator;
pub mod queue_validator;
pub mod tool_call;
pub mod traits;

// Re-export main validation types
pub use errors::{ValidationError, ValidationResult};
pub use traits::{CompositeValidator, ValidatesGenerationRequest, ValidatesToolCall, Validator};

// Re-export validators for easy access
pub use agent_validator::AgentValidator;
pub use mcp_validator::MCPValidator;
pub use queue_validator::{QueueLimits, QueueOperation, QueueValidator};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessageRole, Session, SessionId};
    use std::time::SystemTime;

    /// Simple test validator for testing the trait system
    struct TestValidator {
        should_fail: bool,
    }

    impl Validator<String> for TestValidator {
        type Error = ValidationError;

        fn validate(&self, _session: &Session, target: &String) -> Result<(), Self::Error> {
            if self.should_fail {
                Err(ValidationError::invalid_state(format!(
                    "Test failure for: {}",
                    target
                )))
            } else {
                Ok(())
            }
        }
    }

    /// Create a test session for validation testing
    fn create_test_session() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello, world!".to_string(),
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

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        }
    }

    #[test]
    fn test_validator_trait_success() {
        let validator = TestValidator { should_fail: false };
        let session = create_test_session();
        let target = "test input".to_string();

        assert!(validator.validate(&session, &target).is_ok());
    }

    #[test]
    fn test_validator_trait_failure() {
        let validator = TestValidator { should_fail: true };
        let session = create_test_session();
        let target = "test input".to_string();

        let result = validator.validate(&session, &target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Test failure"));
    }

    #[test]
    fn test_validation_error_constructors() {
        let error = ValidationError::security_violation("test security issue");
        assert!(matches!(error, ValidationError::SecurityViolation(_)));
        assert_eq!(error.to_string(), "Security violation: test security issue\n🔒 Review your input for potentially dangerous content and ensure it follows security guidelines");

        let error = ValidationError::parameter_bounds("test bounds issue");
        assert!(matches!(error, ValidationError::ParameterBounds(_)));
        assert_eq!(
            error.to_string(),
            "Parameter out of bounds: test bounds issue\n📏 Check parameter limits in the documentation and adjust your values accordingly"
        );

        let error = ValidationError::invalid_state("test state issue");
        assert!(matches!(error, ValidationError::InvalidState(_)));
        assert_eq!(error.to_string(), "Invalid state: test state issue\n⚠️ Ensure prerequisites are met and the operation is valid in the current context");

        let error = ValidationError::content_validation("test content issue");
        assert!(matches!(error, ValidationError::ContentValidation(_)));
        assert_eq!(
            error.to_string(),
            "Content validation failed: test content issue\n📝 Verify your content format, encoding, and structure meet the requirements"
        );

        let error = ValidationError::schema_validation("test schema issue");
        assert!(matches!(error, ValidationError::SchemaValidation(_)));
        assert_eq!(
            error.to_string(),
            "Schema validation failed: test schema issue\n📋 Check that your data structure matches the expected schema format"
        );
    }

    #[test]
    fn test_multiple_errors() {
        let errors = vec![
            ValidationError::security_violation("security issue"),
            ValidationError::parameter_bounds("bounds issue"),
        ];

        let combined = ValidationError::multiple(errors.clone());
        assert!(matches!(combined, ValidationError::Multiple(_)));
        assert!(combined.to_string().contains("security issue"));
        assert!(combined.to_string().contains("bounds issue"));

        // Single error should not be wrapped
        let single = ValidationError::multiple(vec![ValidationError::invalid_state("single")]);
        assert!(matches!(single, ValidationError::InvalidState(_)));
        assert_eq!(single.to_string(), "Invalid state: single\n⚠️ Ensure prerequisites are met and the operation is valid in the current context");
    }

    #[test]
    fn test_empty_multiple_errors() {
        let combined = ValidationError::multiple(vec![]);
        // Empty vector creates Multiple variant
        assert!(matches!(combined, ValidationError::Multiple(_)));
        let ValidationError::Multiple(errors) = combined else {
            panic!("Expected Multiple variant")
        };
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validation_error_clone_and_eq() {
        let error1 = ValidationError::security_violation("test");
        let error2 = error1.clone();
        assert_eq!(error1, error2);

        let error3 = ValidationError::parameter_bounds("test");
        assert_ne!(error1, error3);

        let multiple1 = ValidationError::multiple(vec![
            ValidationError::security_violation("a"),
            ValidationError::parameter_bounds("b"),
        ]);
        let multiple2 = ValidationError::multiple(vec![
            ValidationError::security_violation("a"),
            ValidationError::parameter_bounds("b"),
        ]);
        assert_eq!(multiple1, multiple2);
    }

    #[test]
    fn test_validation_result_type() {
        // Test the ValidationResult type alias works correctly
        let success: ValidationResult = Ok(());
        assert!(success.is_ok());

        let failure: ValidationResult = Err(ValidationError::invalid_state("test"));
        assert!(failure.is_err());

        // Test with custom return type
        let success_with_value = "validated".to_string();
        assert_eq!(success_with_value, "validated");
    }

    #[test]
    fn test_validates_generation_request_trait() {
        // TestValidator implements Validator<String, Error = ValidationError>
        // so it should automatically implement ValidatesGenerationRequest<String>
        let validator = TestValidator { should_fail: false };

        // This should compile because of the blanket implementation
        fn accepts_generation_request_validator<T: ValidatesGenerationRequest<String>>(
            _validator: T,
        ) {
        }

        accepts_generation_request_validator(validator);
    }

    #[test]
    fn test_validates_tool_call_trait() {
        // TestValidator implements Validator<String, Error = ValidationError>
        // so it should automatically implement ValidatesToolCall<String>
        let validator = TestValidator { should_fail: false };

        // This should compile because of the blanket implementation
        fn accepts_tool_call_validator<T: ValidatesToolCall<String>>(_validator: T) {}

        accepts_tool_call_validator(validator);
    }
}
