//! Validator execution via ACP agent.
//!
//! This module handles the execution of validators by calling an LLM agent
//! via the Agent Client Protocol (ACP). The LLM evaluates the validator's
//! instructions against the hook event context and returns a pass/fail result.
//!
//! The execution uses the `.validator` prompt template from the prompts library,
//! similar to how rule checking uses the `.check` prompt.

use swissarmyhammer_prompts::PromptLibrary;

use crate::types::HookType;
use crate::validator::{ExecutedValidator, Validator, ValidatorResult};

/// Name of the validator prompt template in the prompts library.
pub const VALIDATOR_PROMPT_NAME: &str = ".validator";

/// Build the rendered prompt for validator execution.
///
/// This renders the `.validator` prompt template with the validator content
/// and hook event context, similar to how rule checking renders `.check`.
pub fn render_validator_prompt(
    prompt_library: &PromptLibrary,
    validator: &Validator,
    hook_type: HookType,
    context: &serde_json::Value,
) -> Result<String, String> {
    use swissarmyhammer_config::TemplateContext;

    let mut template_context = TemplateContext::new();
    template_context.set(
        "validator_content".to_string(),
        validator.body.clone().into(),
    );
    template_context.set(
        "validator_name".to_string(),
        validator.name().to_string().into(),
    );
    template_context.set(
        "hook_context".to_string(),
        serde_json::to_string_pretty(context)
            .unwrap_or_else(|_| context.to_string())
            .into(),
    );
    template_context.set("hook_type".to_string(), hook_type.to_string().into());

    prompt_library
        .render(VALIDATOR_PROMPT_NAME, &template_context)
        .map_err(|e| format!("Failed to render validator prompt: {}", e))
}

/// Parse the LLM response into a ValidatorResult.
///
/// Attempts to extract JSON from the response and parse it as a ValidatorResult.
/// Falls back to creating a failed result if parsing fails.
pub fn parse_validator_response(response: &str) -> ValidatorResult {
    // Try to extract JSON from the response
    let json_str = extract_json(response);

    // First try direct parsing
    if let Ok(result) = serde_json::from_str::<ValidatorResult>(json_str) {
        return result;
    }

    // If that fails, try to find and parse just the first valid JSON object
    // This handles cases where streaming duplicated content
    if let Some(result) = try_parse_first_json_object(response) {
        return result;
    }

    // Last resort: look for status indicators in the text
    let lower = response.to_lowercase();
    if lower.contains("\"status\": \"passed\"") || lower.contains("\"status\":\"passed\"") {
        // Extract message if present
        let message = extract_message_from_response(response)
            .unwrap_or_else(|| "Validation passed".to_string());
        return ValidatorResult::pass(message);
    }
    if lower.contains("\"status\": \"failed\"") || lower.contains("\"status\":\"failed\"") {
        let message = extract_message_from_response(response)
            .unwrap_or_else(|| "Validation failed".to_string());
        return ValidatorResult::fail(message);
    }

    tracing::warn!("Failed to parse validator response as JSON");
    // If we can't parse, assume it failed with the raw response as message
    ValidatorResult::fail(format!(
        "Validator returned invalid JSON response: {}",
        response.chars().take(200).collect::<String>()
    ))
}

/// Try to find and parse the first valid JSON object containing status field.
fn try_parse_first_json_object(response: &str) -> Option<ValidatorResult> {
    // Find the first { and try to extract a complete JSON object
    let mut start = 0;
    while let Some(json_start) = response[start..].find('{') {
        let absolute_start = start + json_start;
        let remaining = &response[absolute_start..];

        // Try to find the matching closing brace
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, c) in remaining.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        let potential_json = &remaining[..=i];
                        // Try to parse this as ValidatorResult
                        if let Ok(result) = serde_json::from_str::<ValidatorResult>(potential_json)
                        {
                            return Some(result);
                        }
                        // Move past this JSON object and try the next one
                        start = absolute_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        // If we didn't find a matching brace, move past this { and try again
        if depth > 0 {
            start = absolute_start + 1;
        }
    }
    None
}

/// Extract the message field from a response containing JSON.
fn extract_message_from_response(response: &str) -> Option<String> {
    // Look for "message": "..." pattern
    let patterns = [r#""message": ""#, r#""message":""#];

    for pattern in patterns {
        if let Some(start) = response.find(pattern) {
            let after_key = &response[start + pattern.len()..];
            // Find the closing quote (handling escaped quotes)
            let mut end = 0;
            let mut escape_next = false;
            for (i, c) in after_key.char_indices() {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                match c {
                    '\\' => escape_next = true,
                    '"' => {
                        end = i;
                        break;
                    }
                    _ => {}
                }
            }
            if end > 0 {
                return Some(after_key[..end].to_string());
            }
        }
    }
    None
}

/// Extract JSON from a response that might have surrounding text.
///
/// Looks for JSON object delimiters and extracts the content.
/// Handles various edge cases including malformed markdown blocks.
fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();

    // Try to find JSON within markdown code blocks first
    if let Some(start) = trimmed.find("```json") {
        let after_marker = &trimmed[start + 7..];
        if let Some(end) = after_marker.find("```") {
            let json_content = after_marker[..end].trim();
            // Validate it looks like JSON before returning
            if json_content.starts_with('{') && json_content.ends_with('}') {
                return json_content;
            }
        }
    }

    // Try to find bare code block
    if let Some(start) = trimmed.find("```") {
        let after_marker = &trimmed[start + 3..];
        // Skip optional language identifier on the same line
        let content_start = after_marker.find('\n').map(|i| i + 1).unwrap_or(0);
        let content = &after_marker[content_start..];
        if let Some(end) = content.find("```") {
            let json_content = content[..end].trim();
            // Validate it looks like JSON before returning
            if json_content.starts_with('{') && json_content.ends_with('}') {
                return json_content;
            }
        }
    }

    // If it starts with {, try to find the matching } using bracket counting
    if trimmed.starts_with('{') {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, c) in trimmed.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return &trimmed[..=i];
                    }
                }
                _ => {}
            }
        }
    }

    // Last resort: find the first { and last } and hope for the best
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return &trimmed[start..=end];
        }
    }

    // Return as-is and let serde handle it
    trimmed
}

/// Create an ExecutedValidator from a validator and its result.
pub fn create_executed_validator(
    validator: &Validator,
    result: ValidatorResult,
) -> ExecutedValidator {
    ExecutedValidator {
        name: validator.name().to_string(),
        severity: validator.severity(),
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_validator_response_passed() {
        let response = r#"{"status": "passed", "message": "All checks passed"}"#;
        let result = parse_validator_response(response);
        assert!(result.passed());
        assert_eq!(result.message(), "All checks passed");
    }

    #[test]
    fn test_parse_validator_response_failed() {
        let response = r#"{"status": "failed", "message": "Found a secret on line 42"}"#;
        let result = parse_validator_response(response);
        assert!(!result.passed());
        assert_eq!(result.message(), "Found a secret on line 42");
    }

    #[test]
    fn test_parse_validator_response_with_markdown() {
        let response = r#"
Here's my analysis:

```json
{"status": "passed", "message": "No issues found"}
```
"#;
        let result = parse_validator_response(response);
        assert!(result.passed());
        assert_eq!(result.message(), "No issues found");
    }

    #[test]
    fn test_parse_validator_response_invalid_json() {
        let response = "This is not JSON at all";
        let result = parse_validator_response(response);
        assert!(!result.passed());
        assert!(result.message().contains("invalid JSON"));
    }

    #[test]
    fn test_extract_json_bare() {
        let input = r#"{"status": "passed", "message": "OK"}"#;
        assert_eq!(extract_json(input), input);
    }

    #[test]
    fn test_extract_json_with_whitespace() {
        let input = r#"
        {"status": "passed", "message": "OK"}
        "#;
        assert_eq!(
            extract_json(input),
            r#"{"status": "passed", "message": "OK"}"#
        );
    }

    #[test]
    fn test_extract_json_markdown_block() {
        let input = r#"```json
{"status": "failed", "message": "Bad"}
```"#;
        assert_eq!(
            extract_json(input),
            r#"{"status": "failed", "message": "Bad"}"#
        );
    }

    #[test]
    fn test_render_validator_prompt() {
        use crate::validator::{Severity, ValidatorFrontmatter, ValidatorSource};
        use std::path::PathBuf;

        let validator = Validator {
            frontmatter: ValidatorFrontmatter {
                name: "test".to_string(),
                description: "Test validator".to_string(),
                severity: Severity::Error,
                trigger: HookType::PreToolUse,
                match_criteria: None,
                trigger_matcher: None,
                tags: vec![],
                once: false,
                timeout: 30,
            },
            body: "Check for issues in the code.".to_string(),
            source: ValidatorSource::Builtin,
            path: PathBuf::from("test.md"),
        };

        let context = serde_json::json!({
            "tool_name": "Write",
            "file_path": "test.ts"
        });

        // Create a prompt library with builtins loaded
        let mut resolver = swissarmyhammer_prompts::PromptResolver::new();
        let mut prompt_library = PromptLibrary::new();
        resolver.load_all_prompts(&mut prompt_library).unwrap();

        let prompt =
            render_validator_prompt(&prompt_library, &validator, HookType::PreToolUse, &context);

        assert!(prompt.is_ok());
        let prompt_text = prompt.unwrap();
        assert!(prompt_text.contains("Check for issues in the code."));
        assert!(prompt_text.contains("PreToolUse"));
        assert!(prompt_text.contains("tool_name"));
    }
}
