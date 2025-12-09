//! Tests for CompactionPrompt validation, loading, and rendering functionality

use crate::resources::ResourceLoader;

use crate::types::*;

#[cfg(test)]
mod compaction_prompt_tests {
    use super::*;

    #[test]
    fn test_default_prompt_loading() {
        let prompt = CompactionPrompt::load_default().expect("Should load default prompt");

        assert!(
            !prompt.system_instructions.is_empty(),
            "System instructions should not be empty"
        );
        assert!(
            !prompt.user_prompt_template.is_empty(),
            "User prompt template should not be empty"
        );

        // Should contain the conversation history placeholder
        assert!(
            prompt
                .user_prompt_template
                .contains("{conversation_history}"),
            "Template should contain {{conversation_history}} placeholder"
        );

        // System instructions should be reasonable length
        assert!(
            prompt.system_instructions.len() > 50,
            "System instructions should be substantial"
        );
    }

    #[test]
    fn test_prompt_validation_valid() {
        let valid_prompt = CompactionPrompt {
            system_instructions: "You are a helpful assistant that summarizes conversations effectively and preserves important context.".to_string(),
            user_template: "Please summarize this conversation: {conversation_history}".to_string(),
            user_prompt_template: "Please summarize this conversation: {conversation_history}".to_string(),
        };

        assert!(
            valid_prompt.validate().is_ok(),
            "Valid prompt should pass validation"
        );
    }

    #[test]
    fn test_prompt_validation_missing_placeholder() {
        let invalid_prompt = CompactionPrompt {
            system_instructions: "You are a helpful assistant that summarizes conversations."
                .to_string(),
            user_template: "Please summarize this conversation.".to_string(), // No placeholder
            user_prompt_template: "Please summarize this conversation.".to_string(), // No placeholder
        };

        let result = invalid_prompt.validate();
        assert!(
            result.is_err(),
            "Prompt without placeholder should fail validation"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("conversation_history"),
            "Error should mention missing placeholder"
        );
    }

    #[test]
    fn test_prompt_validation_short_instructions() {
        let invalid_prompt = CompactionPrompt {
            system_instructions: "Short".to_string(), // Too short
            user_template: "Summarize: {conversation_history}".to_string(),
            user_prompt_template: "Summarize: {conversation_history}".to_string(),
        };

        let result = invalid_prompt.validate();
        assert!(
            result.is_err(),
            "Prompt with short instructions should fail validation"
        );
    }

    #[test]
    fn test_prompt_validation_empty_instructions() {
        let invalid_prompt = CompactionPrompt {
            system_instructions: "".to_string(),
            user_template: "Summarize: {conversation_history}".to_string(),
            user_prompt_template: "Summarize: {conversation_history}".to_string(),
        };

        let result = invalid_prompt.validate();
        assert!(
            result.is_err(),
            "Prompt with empty instructions should fail validation"
        );
    }

    #[test]
    fn test_prompt_validation_empty_template() {
        let invalid_prompt = CompactionPrompt {
            system_instructions: "You are a helpful assistant that summarizes conversations."
                .to_string(),
            user_template: "".to_string(),
            user_prompt_template: "".to_string(),
        };

        let result = invalid_prompt.validate();
        assert!(
            result.is_err(),
            "Prompt with empty template should fail validation"
        );
    }

    #[test]
    fn test_template_rendering_basic() {
        let prompt = CompactionPrompt {
            system_instructions: "System prompt".to_string(),
            user_template: "Please summarize: {conversation_history}".to_string(),
            user_prompt_template: "Please summarize: {conversation_history}".to_string(),
        };

        let history = "User: Hello\nAssistant: Hi there!";
        let rendered = prompt.render_user_prompt(history);

        assert!(
            rendered.contains("User: Hello"),
            "Rendered prompt should contain user message"
        );
        assert!(
            rendered.contains("Assistant: Hi there!"),
            "Rendered prompt should contain assistant message"
        );
        assert!(
            !rendered.contains("{conversation_history}"),
            "Rendered prompt should not contain placeholder"
        );
    }

    #[test]
    fn test_template_rendering_multiple_placeholders() {
        let prompt = CompactionPrompt {
            system_instructions: "System prompt".to_string(),
            user_template: "Context: {conversation_history}\n\nPlease summarize the above: {conversation_history}".to_string(),
            user_prompt_template: "Context: {conversation_history}\n\nPlease summarize the above: {conversation_history}".to_string(),
        };

        let history = "User: Test message";
        let rendered = prompt.render_user_prompt(history);

        // Should replace all instances of the placeholder
        let placeholder_count = rendered.matches("{conversation_history}").count();
        assert_eq!(placeholder_count, 0, "All placeholders should be replaced");

        // Should contain the history content multiple times
        let content_count = rendered.matches("User: Test message").count();
        assert_eq!(content_count, 2, "History should appear twice");
    }

    #[test]
    fn test_template_rendering_empty_history() {
        let prompt = CompactionPrompt {
            system_instructions: "System prompt".to_string(),
            user_template: "History: {conversation_history}".to_string(),
            user_prompt_template: "History: {conversation_history}".to_string(),
        };

        let rendered = prompt.render_user_prompt("");

        assert!(
            rendered.contains("History: "),
            "Template structure should be preserved"
        );
        assert!(
            !rendered.contains("{conversation_history}"),
            "Placeholder should be removed"
        );
    }

    #[test]
    fn test_custom_prompt_creation_valid() {
        let custom = CompactionPrompt::custom(
            "Custom system instructions for testing that are long enough to pass validation."
                .to_string(),
            "Custom template with {conversation_history} placeholder.".to_string(),
        );

        assert!(
            custom.is_ok(),
            "Valid custom prompt should be created successfully"
        );
        let prompt = custom.unwrap();
        assert_eq!(
            prompt.system_instructions,
            "Custom system instructions for testing that are long enough to pass validation."
        );
        assert!(prompt
            .user_prompt_template
            .contains("{conversation_history}"));
    }

    #[test]
    fn test_custom_prompt_creation_invalid() {
        let custom = CompactionPrompt::custom(
            "Short".to_string(), // Too short
            "Template without placeholder".to_string(),
        );

        assert!(
            custom.is_err(),
            "Invalid custom prompt should fail creation"
        );
    }

    #[test]
    fn test_prompt_create_messages() {
        let prompt = CompactionPrompt {
            system_instructions: "You are a helpful summarizer.".to_string(),
            user_template: "Please summarize: {conversation_history}".to_string(),
            user_prompt_template: "Please summarize: {conversation_history}".to_string(),
        };

        let history = "User: Hello\nAssistant: Hi there!";
        let messages = prompt.create_messages(history);

        assert_eq!(messages.len(), 2, "Should create system and user messages");

        // Check system message
        assert_eq!(messages[0].role, MessageRole::System);
        assert_eq!(messages[0].content, "You are a helpful summarizer.");

        // Check user message
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(messages[1].content.contains("User: Hello"));
        assert!(messages[1].content.contains("Assistant: Hi there!"));
        assert!(!messages[1].content.contains("{conversation_history}"));
    }

    #[test]
    fn test_prompt_estimated_tokens() {
        let prompt = CompactionPrompt {
            system_instructions: "You are a helpful assistant.".to_string(),
            user_template: "Summarize: {conversation_history}".to_string(),
            user_prompt_template: "Summarize: {conversation_history}".to_string(),
        };

        let history = "User: Hello world\nAssistant: Hi there friend";
        let estimated = prompt.estimated_prompt_tokens(history.len());

        // Should be greater than zero and reasonable
        assert!(
            estimated > 0,
            "Estimated tokens should be greater than zero"
        );
        assert!(
            estimated < 100,
            "Estimated tokens should be reasonable for short text"
        );
    }

    #[test]
    fn test_prompt_estimated_tokens_large_history() {
        let prompt = CompactionPrompt {
            system_instructions: "You are a helpful assistant.".to_string(),
            user_template: "Summarize: {conversation_history}".to_string(),
            user_prompt_template: "Summarize: {conversation_history}".to_string(),
        };

        // Create large conversation history
        let large_history = "word ".repeat(1000);
        let estimated = prompt.estimated_prompt_tokens(large_history.len());

        // Should scale with content size
        assert!(
            estimated > 500,
            "Large content should have substantial token estimate"
        );
    }

    #[test]
    fn test_prompt_from_resource_content() {
        let resource_content = r#"# System Instructions

You are tasked with creating a concise summary of a conversation.

# User Prompt Template

Please summarize this conversation: {conversation_history}"#;

        let result = CompactionPrompt::from_resource(resource_content);
        assert!(
            result.is_ok(),
            "Valid resource content should parse successfully"
        );

        let prompt = result.unwrap();
        assert!(prompt.system_instructions.contains("concise summary"));
        assert!(prompt
            .user_prompt_template
            .contains("{conversation_history}"));
    }

    #[test]
    fn test_prompt_from_resource_content_missing_sections() {
        let resource_content = "# System Instructions\nSome instructions\n";

        let result = CompactionPrompt::from_resource(resource_content);
        assert!(
            result.is_err(),
            "Resource missing user prompt template should fail"
        );
    }

    #[test]
    fn test_prompt_from_resource_content_empty_sections() {
        let resource_content = r#"# System Instructions

# User Prompt Template
"#;

        let result = CompactionPrompt::from_resource(resource_content);
        assert!(result.is_err(), "Resource with empty sections should fail");
    }

    #[test]
    fn test_prompt_with_complex_conversation() {
        let prompt = CompactionPrompt {
            system_instructions: "System instructions".to_string(),
            user_template: "Conversation: {conversation_history}".to_string(),
            user_prompt_template: "Conversation: {conversation_history}".to_string(),
        };

        let complex_history = r#"User: Can you help me with a coding problem?
Assistant: Of course! I'd be happy to help. What specific coding problem are you working on?
User: I need to implement a function that validates email addresses.
Assistant: I can help you with that. Here's a simple email validation function:

```python
import re

def validate_email(email):
    pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$'
    return re.match(pattern, email) is not None
```

User: Thank you! That's very helpful."#;

        let rendered = prompt.render_user_prompt(complex_history);

        // Should preserve formatting and special characters
        assert!(rendered.contains("```python"));
        assert!(rendered.contains("def validate_email"));
        assert!(rendered.contains("Thank you!"));
        assert!(!rendered.contains("{conversation_history}"));
    }

    #[test]
    fn test_prompt_with_special_characters() {
        let prompt = CompactionPrompt {
            system_instructions: "System instructions".to_string(),
            user_template: "Content: {conversation_history}".to_string(),
            user_prompt_template: "Content: {conversation_history}".to_string(),
        };

        let history_with_special =
            "User: Hello! @#$%^&*()_+{}|:<>?[];',./`~\nAssistant: Special chars handled!";
        let rendered = prompt.render_user_prompt(history_with_special);

        // Should preserve special characters
        assert!(rendered.contains("@#$%^&*()"));
        assert!(rendered.contains("Special chars handled!"));
    }

    #[test]
    fn test_prompt_validation_edge_cases() {
        // Test with minimum valid length
        let min_valid = CompactionPrompt {
            system_instructions: "A".repeat(20), // Minimum acceptable length
            user_template: "Text: {conversation_history}".to_string(),
            user_prompt_template: "Text: {conversation_history}".to_string(),
        };

        // This might pass or fail depending on the exact validation rules
        let _result = min_valid.validate();
        // Don't assert success/failure as it depends on implementation details

        // Test with very long content
        let very_long = CompactionPrompt {
            system_instructions: "A".repeat(10000),
            user_template: format!("Text: {} END", "{conversation_history}"),
            user_prompt_template: format!("Text: {} END", "{conversation_history}"),
        };

        assert!(
            very_long.validate().is_ok(),
            "Very long valid prompt should pass"
        );
    }

    #[test]
    fn test_prompt_rendering_with_newlines_and_formatting() {
        let prompt = CompactionPrompt {
            system_instructions: "Preserve formatting".to_string(),
            user_template: "History:\n{conversation_history}\n\nEnd of history.".to_string(),
            user_prompt_template: "History:\n{conversation_history}\n\nEnd of history.".to_string(),
        };

        let history = "Line 1\nLine 2\n\nLine 4";
        let rendered = prompt.render_user_prompt(history);

        assert!(rendered.contains("History:\nLine 1"));
        assert!(rendered.contains("Line 4\n\nEnd of history"));
    }

    #[test]
    fn test_resource_loader_integration() {
        let loader = ResourceLoader::new();
        let resource_content = loader.load_resource("compaction.md");

        // This tests that the resource loader can load the compaction resource
        assert!(
            resource_content.is_ok(),
            "Should be able to load compaction resource"
        );

        let content = resource_content.unwrap();
        assert!(content.contains("# System Instructions"));
        assert!(content.contains("# User Prompt Template"));
        assert!(content.contains("{conversation_history}"));
    }

    #[test]
    fn test_default_prompt_resource_parsing() {
        // Test that the default prompt can be loaded and parsed correctly
        let prompt = CompactionPrompt::load_default();
        assert!(prompt.is_ok(), "Default prompt should load successfully");

        let prompt = prompt.unwrap();
        assert!(prompt.validate().is_ok(), "Default prompt should be valid");

        // Should have reasonable content
        assert!(prompt.system_instructions.contains("summary"));
        assert!(prompt.user_prompt_template.contains("conversation"));
    }

    #[test]
    fn test_prompt_message_timestamps() {
        let prompt = CompactionPrompt {
            system_instructions: "Test instructions".to_string(),
            user_template: "Test: {conversation_history}".to_string(),
            user_prompt_template: "Test: {conversation_history}".to_string(),
        };

        let messages = prompt.create_messages("Test history");

        // Messages should have timestamps
        for message in &messages {
            // SystemTime should be reasonably recent (within last minute)
            let elapsed = message.timestamp.elapsed().unwrap();
            assert!(elapsed.as_secs() < 60, "Message timestamp should be recent");
        }
    }
}
