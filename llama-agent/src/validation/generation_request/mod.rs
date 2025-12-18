//! Generation request validation components
//!
//! This module contains all validators related to GenerationRequest validation.

mod composite_validator;
mod message_validator;
mod session_validator;

pub use composite_validator::{CompositeGenerationRequestValidator, ValidationConfig};
pub use message_validator::{MessageContentConfig, MessageContentValidator};

pub use session_validator::SessionStateValidator;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::types::{GenerationRequest, Message, MessageRole, Session, SessionId};
    use crate::validation::Validator;
    // ValidationLimits not available in llama_common
    use std::time::SystemTime;

    #[test]
    fn test_session_validator_integration() {
        let validator = SessionStateValidator::new();

        // Create a realistic session
        let session = Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "What is the weather like?".to_string(),
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
            template_token_count: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
        };

        // Create a realistic generation request
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(150),
            temperature: Some(0.8),
            top_p: None,
            stop_tokens: vec!["Human:".to_string()],
            stopping_config: None,
        };

        // Validation should pass
        assert!(validator.validate(&session, &request).is_ok());
    }

    #[test]
    fn test_full_generation_request_validation_pipeline() {
        // Test the complete validation pipeline with realistic data
        let validator = CompositeGenerationRequestValidator::new();

        let session = Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "You are a helpful assistant specializing in programming help.".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now() - std::time::Duration::from_secs(120),
                },
                Message {
                    role: MessageRole::User,
                    content: "I need help with error handling in Rust. Can you show me best practices?".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now() - std::time::Duration::from_secs(60),
                },
                Message {
                    role: MessageRole::Assistant,
                    content: "I'd be happy to help with Rust error handling! Here are the key best practices:".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now() - std::time::Duration::from_secs(30),
                },
            ],
            mcp_servers: vec![],
            available_tools: vec![],
            available_prompts: vec![],
            created_at: SystemTime::now() - std::time::Duration::from_secs(180),
            updated_at: SystemTime::now() - std::time::Duration::from_secs(30),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,
            template_token_count: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

        client_capabilities: None,
        };

        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(800),
            temperature: Some(0.8),
            top_p: Some(0.95),
            stop_tokens: vec!["User:".to_string(), "Human:".to_string()],
            stopping_config: None,
        };

        // This should pass all validation stages
        let result = validator.validate(&session, &request);
        assert!(result.is_ok(), "Validation failed: {:?}", result);
    }

    #[test]
    fn test_composite_validator_error_priority() {
        // Test validation order - session validation should fail before message validation
        let validator = CompositeGenerationRequestValidator::new();

        // Create session with no messages AND suspicious content in a hypothetical message
        let session = Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![], // Empty messages - should fail session validation first
            mcp_servers: vec![],
            available_tools: vec![],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,
            template_token_count: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
        };

        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(800),
            temperature: Some(0.8),
            top_p: Some(0.95),
            stop_tokens: vec!["User:".to_string()],
            stopping_config: None,
        };

        let result = validator.validate(&session, &request);
        assert!(result.is_err());
        // Should fail on session validation (empty messages), not get to other validations
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one message"));
    }

    #[test]
    fn test_composite_validator_with_custom_config() {
        // Test composite validator with custom configuration
        let config = ValidationConfig {
            message_content: MessageContentConfig {
                // No configuration needed - all validation handled by llama-cpp
            },
        };

        let validator = CompositeGenerationRequestValidator::with_config(config);

        // Test with message that's too long for the custom config
        let session = Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "This message is definitely longer than 50 characters and should fail the custom validation config".to_string(),
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
            template_token_count: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

        client_capabilities: None,
        };

        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.8),
            top_p: Some(0.5),
            stop_tokens: vec!["stop".to_string()],
            stopping_config: None,
        };

        let result = validator.validate(&session, &request);
        // Since parameter validation is no longer done by composite validator,
        // this request now passes validation as all validation is deferred to llama-cpp
        // Parameter validation is now handled by GenerationConfig::validate()
        assert!(result.is_ok());
    }
}
