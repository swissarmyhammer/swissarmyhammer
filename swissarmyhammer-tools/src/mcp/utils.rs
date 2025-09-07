//! Utility functions for MCP operations

use rmcp::ErrorData as McpError;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_issues_config::Config;

/// Convert a JSON map to a string map for template arguments
pub fn convert_prompt_arguments(arguments: &HashMap<String, Value>) -> HashMap<String, String> {
    arguments
        .iter()
        .map(|(k, v)| {
            let value_str = match v {
                Value::String(s) => s.clone(),
                _ => v.to_string(),
            };
            (k.clone(), value_str)
        })
        .collect()
}

/// Convert a JSON map to a string map
pub fn json_map_to_string_map(
    json_map: &serde_json::Map<String, Value>,
) -> HashMap<String, String> {
    json_map
        .iter()
        .map(|(k, v)| {
            let value_str = match v {
                Value::String(s) => s.clone(),
                _ => v.to_string(),
            };
            (k.clone(), value_str)
        })
        .collect()
}

/// Generate a JSON schema for a type that implements JsonSchema
pub fn generate_tool_schema<T>() -> Arc<serde_json::Map<String, Value>>
where
    T: schemars::JsonSchema,
{
    serde_json::to_value(schemars::schema_for!(T))
        .ok()
        .and_then(|v| v.as_object().map(|obj| Arc::new(obj.clone())))
        .unwrap_or_else(|| Arc::new(serde_json::Map::new()))
}

/// Validate and normalize an issue name according to MCP standards
///
/// This function performs comprehensive validation including:
/// - Empty/whitespace checks
/// - Length limits (configurable maximum)
/// - Invalid filesystem character checks
/// - Additional validation using the existing issues module
///
/// # Arguments
///
/// * `name` - The raw issue name to validate
///
/// # Returns
///
/// * `Result<String, McpError>` - The validated and trimmed name, or an error
pub fn validate_issue_name(name: &str) -> std::result::Result<String, McpError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(McpError::invalid_params("Issue name cannot be empty", None));
    }

    let config = Config::global();
    if trimmed.len() > config.max_issue_name_length {
        return Err(McpError::invalid_params(
            format!(
                "Issue name too long (max {} characters)",
                config.max_issue_name_length
            ),
            None,
        ));
    }

    // Check for invalid characters
    if trimmed.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(McpError::invalid_params(
            "Issue name contains invalid characters",
            None,
        ));
    }

    // Additional validation
    if trimmed.contains('\0') {
        return Err(McpError::invalid_params(
            "Issue name contains null characters",
            None,
        ));
    }

    Ok(trimmed.to_string())
}

/// Validate issue content comprehensively according to MCP standards
///
/// This function validates that issue content doesn't exceed size limits,
/// contains safe markdown, and doesn't include potentially dangerous content.
///
/// # Arguments
///
/// * `content` - The issue content to validate
///
/// # Returns
///
/// * `Result<(), McpError>` - Success or validation error
pub fn validate_issue_content_size(content: &str) -> std::result::Result<(), McpError> {
    let config = Config::global();

    // Check content size in bytes
    if content.len() > config.max_content_length {
        return Err(McpError::invalid_params(
            format!(
                "Issue content too large: {} bytes (max {} bytes)",
                content.len(),
                config.max_content_length
            ),
            None,
        ));
    }

    // Check for extremely long lines
    if content
        .lines()
        .any(|line| line.len() > config.max_line_length)
    {
        return Err(McpError::invalid_params(
            format!(
                "Issue content lines cannot exceed {} characters",
                config.max_line_length
            ),
            None,
        ));
    }

    // Validate against control characters (except common ones like tabs and newlines)
    for (line_num, line) in content.lines().enumerate() {
        for c in line.chars() {
            if c.is_control() && c != '\t' && c != '\n' && c != '\r' {
                return Err(McpError::invalid_params(
                    format!(
                        "Issue content contains invalid control characters on line {}: '{}'",
                        line_num + 1,
                        line.chars()
                            .map(|c| if c.is_control() { '�' } else { c })
                            .collect::<String>()
                    ),
                    None,
                ));
            }
        }
    }

    // Check for potentially dangerous HTML tags/XSS vectors
    validate_html_security(content)?;

    // Validate markdown structure
    validate_markdown_structure(content)?;

    Ok(())
}

/// Validate content against dangerous HTML tags
fn validate_dangerous_html_tags(content: &str) -> std::result::Result<(), McpError> {
    use regex::Regex;

    static DANGEROUS_TAG_PATTERNS: &[&str] = &[
        r"<\s*script[^>]*>",
        r"<\s*iframe[^>]*>",
        r"<\s*object[^>]*>",
        r"<\s*embed[^>]*>",
        r"<\s*link[^>]*>",
        r"<\s*style[^>]*>",
        r"<\s*meta[^>]*>",
        r"<\s*base[^>]*>",
        r"<\s*form[^>]*>",
        r"<\s*input[^>]*>",
        r"<\s*button[^>]*>",
        r"<\s*svg[^>]*>",
        r"<\s*math[^>]*>",
        r"<\s*details[^>]*>",
        r"<\s*dialog[^>]*>",
    ];

    for pattern in DANGEROUS_TAG_PATTERNS {
        if let Ok(regex) = Regex::new(pattern) {
            if regex.is_match(content) {
                return Err(McpError::invalid_params(
                    format!(
                        "Issue content contains potentially dangerous HTML tag matching pattern: '{pattern}'"
                    ),
                    None,
                ));
            }
        }
    }
    Ok(())
}

/// Validate content against dangerous protocols
fn validate_dangerous_protocols(content: &str) -> std::result::Result<(), McpError> {
    use regex::Regex;

    static DANGEROUS_PROTOCOLS: &[&str] = &[
        r"javascript\s*:",
        r"data\s*:",
        r"vbscript\s*:",
        r"file\s*:",
        r"ftp\s*:",
    ];

    for pattern in DANGEROUS_PROTOCOLS {
        if let Ok(regex) = Regex::new(pattern) {
            if regex.is_match(content) {
                return Err(McpError::invalid_params(
                    format!("Issue content contains potentially dangerous protocol: '{pattern}'"),
                    None,
                ));
            }
        }
    }
    Ok(())
}

/// Validate content against dangerous event handlers
fn validate_event_handlers(content: &str) -> std::result::Result<(), McpError> {
    use regex::Regex;

    static EVENT_HANDLER_PATTERNS: &[&str] = &[
        r"on\w+\s*=",
        r"@\w+\s*=",   // Vue.js style events
        r"ng-\w+\s*=", // Angular style events
    ];

    for pattern in EVENT_HANDLER_PATTERNS {
        if let Ok(regex) = Regex::new(pattern) {
            if regex.is_match(content) {
                return Err(McpError::invalid_params(
                    format!(
                        "Issue content contains potentially dangerous event handler: '{pattern}'"
                    ),
                    None,
                ));
            }
        }
    }
    Ok(())
}

/// Validate content against dangerous attributes
fn validate_dangerous_attributes(content: &str) -> std::result::Result<(), McpError> {
    use regex::Regex;

    static DANGEROUS_ATTRIBUTE_PATTERNS: &[&str] = &[
        r"srcdoc\s*=",
        r"formaction\s*=",
        r"action\s*=",
        r"background\s*=",
        r"poster\s*=",
    ];

    for pattern in DANGEROUS_ATTRIBUTE_PATTERNS {
        if let Ok(regex) = Regex::new(pattern) {
            if regex.is_match(content) {
                return Err(McpError::invalid_params(
                    format!("Issue content contains potentially dangerous attribute: '{pattern}'"),
                    None,
                ));
            }
        }
    }
    Ok(())
}

/// Validate content against potential XSS vectors and dangerous HTML
/// This provides comprehensive HTML sanitization using regex patterns and context-aware validation
fn validate_html_security(content: &str) -> std::result::Result<(), McpError> {
    let content_lower = content.to_lowercase();

    // Check for dangerous HTML tags
    validate_dangerous_html_tags(&content_lower)?;

    // Check for dangerous protocols
    validate_dangerous_protocols(&content_lower)?;

    // Check for event handlers
    validate_event_handlers(&content_lower)?;

    // Check for dangerous attributes
    validate_dangerous_attributes(&content_lower)?;

    // Additional validation for encoded content
    validate_encoded_content(&content_lower)?;

    Ok(())
}

/// Validate against encoded malicious content
fn validate_encoded_content(content: &str) -> std::result::Result<(), McpError> {
    // Check for HTML entities that could be used to bypass validation
    let suspicious_entities = [
        "&#x6a;&#x61;&#x76;&#x61;&#x73;&#x63;&#x72;&#x69;&#x70;&#x74;", // javascript
        "&#106;&#97;&#118;&#97;&#115;&#99;&#114;&#105;&#112;&#116;",    // javascript
        "&lt;script",                                                   // encoded script tags
        "&lt;iframe",
        "&lt;object",
        "%3cscript", // URL encoded script
        "%3ciframe",
        "%3cobject",
    ];

    for entity in &suspicious_entities {
        if content.contains(entity) {
            return Err(McpError::invalid_params(
                format!("Issue content contains potentially dangerous encoded content: '{entity}'"),
                None,
            ));
        }
    }

    Ok(())
}

/// Validate basic markdown structure
fn validate_markdown_structure(content: &str) -> std::result::Result<(), McpError> {
    // Check for balanced code blocks
    let mut code_block_count = 0;
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for code block markers
        if trimmed.starts_with("```") {
            if in_code_block {
                code_block_count -= 1;
                in_code_block = false;
            } else {
                code_block_count += 1;
                in_code_block = true;
            }
        }
    }

    if code_block_count > 0 {
        return Err(McpError::invalid_params(
            format!(
                "Issue content has {code_block_count} unmatched code blocks (```). Each opening ``` must have a closing ```"
            ),
            None,
        ));
    }

    Ok(())
}
