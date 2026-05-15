//! Message content validation for generation requests

use crate::types::{Message, Session};
use crate::validation::{ValidationError, ValidationResult, Validator};

/// Configuration for message content validation
///
/// Currently contains no configuration options as all validation
/// is deferred to llama-cpp for handling. This struct exists to
/// maintain consistent API patterns across validators.
#[derive(Debug, Clone, Default)]
pub struct MessageContentConfig {
    // No configuration needed - all validation handled by llama-cpp
}

/// Validates message content for basic issues
///
/// Currently performs minimal validation as llama-cpp handles parameter validation
#[derive(Debug, Clone)]
pub struct MessageContentValidator {}

impl MessageContentValidator {
    /// Create a new message content validator with default settings
    pub fn new() -> Self {
        Self {}
    }

    /// Create a validator with custom configuration
    pub fn with_config(_config: MessageContentConfig) -> Self {
        Self {}
    }
}

impl Default for MessageContentValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator<Message> for MessageContentValidator {
    type Error = ValidationError;

    fn validate(&self, _session: &Session, _message: &Message) -> ValidationResult {
        // No validation needed - llama-cpp handles all parameter validation
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::centralized_test_utils::*;

    #[test]
    fn test_valid_message_passes() {
        let validator = MessageContentValidator::new();
        let session = create_empty_session();
        let message = create_test_message("Hello, how are you today?");

        assert!(validator.validate(&session, &message).is_ok());
    }

    #[test]
    fn test_long_message_passes() {
        let validator = MessageContentValidator::new();
        let session = create_empty_session();
        let long_content = "a".repeat(100_001); // Long message should now pass
        let message = create_test_message(&long_content);

        let result = validator.validate(&session, &message);
        assert!(result.is_ok()); // Should pass since llama-cpp handles validation
    }

    #[test]
    fn test_custom_config() {
        let config = MessageContentConfig::default();
        let validator = MessageContentValidator::with_config(config);
        let session = create_empty_session();

        // Test that any message passes with custom config
        let long_message = create_test_message(&"a".repeat(1001));
        let result = validator.validate(&session, &long_message);
        assert!(result.is_ok()); // Should pass since llama-cpp handles validation
    }

    #[test]
    fn test_validator_is_cloneable() {
        let validator1 = MessageContentValidator::new();
        let validator2 = validator1.clone();

        let session = create_empty_session();
        let message = create_test_message("test message");

        // Both should work identically
        assert_eq!(
            validator1.validate(&session, &message).is_ok(),
            validator2.validate(&session, &message).is_ok()
        );
    }

    #[test]
    fn test_empty_message_passes() {
        let validator = MessageContentValidator::new();
        let session = create_empty_session();
        let message = create_test_message("");

        assert!(validator.validate(&session, &message).is_ok());
    }

    #[test]
    fn test_whitespace_only_message_passes() {
        let validator = MessageContentValidator::new();
        let session = create_empty_session();
        let message = create_test_message("   \t\n  ");

        assert!(validator.validate(&session, &message).is_ok());
    }
}
