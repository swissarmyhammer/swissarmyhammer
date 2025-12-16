//! YAML frontmatter parsing for mode files
//!
//! This module provides functionality to parse YAML frontmatter from
//! markdown files containing mode definitions.

use crate::Result;
use serde_json::Value;
use swissarmyhammer_common::SwissArmyHammerError;

/// Represents parsed frontmatter and content
#[derive(Debug, Clone)]
pub struct FrontmatterResult {
    /// Parsed YAML metadata (None if no frontmatter)
    pub metadata: Option<Value>,
    /// Remaining content after frontmatter removal (the system prompt)
    pub content: String,
}

/// Parse YAML frontmatter from content
///
/// Expects frontmatter to be delimited by `---` at the beginning and end,
/// followed by the system prompt content.
///
/// # Format
/// ```markdown
/// ---
/// name: example-mode
/// description: An example mode
/// ---
/// System prompt goes here
/// ```
pub fn parse_frontmatter(content: &str) -> Result<FrontmatterResult> {
    let content = content.trim_start();

    // Check if content starts with frontmatter delimiter
    if !content.starts_with("---") {
        // No frontmatter, return the entire content
        return Ok(FrontmatterResult {
            metadata: None,
            content: content.to_string(),
        });
    }

    // Find the closing delimiter
    let after_first_delimiter = &content[3..]; // Skip the first "---"

    // Look for the line ending after the first ---
    let start_pos = if after_first_delimiter.starts_with('\n') {
        4 // "---\n"
    } else if after_first_delimiter.starts_with("\r\n") {
        5 // "---\r\n"
    } else {
        // No line ending after first delimiter, treat as no frontmatter
        return Ok(FrontmatterResult {
            metadata: None,
            content: content.to_string(),
        });
    };

    // Find the closing "---" on its own line
    if let Some(end_pos) = find_closing_delimiter(&content[start_pos..]) {
        let yaml_content = &content[start_pos..start_pos + end_pos];
        let remaining_content = &content[start_pos + end_pos..];

        // Skip the closing delimiter and any following newlines
        let remaining_content = remaining_content
            .strip_prefix("---")
            .unwrap_or(remaining_content)
            .trim_start_matches('\r')
            .trim_start_matches('\n');

        // Parse the YAML content
        let metadata = if yaml_content.trim().is_empty() {
            None
        } else {
            match serde_yaml::from_str::<Value>(yaml_content) {
                Ok(value) => Some(value),
                Err(e) => {
                    return Err(SwissArmyHammerError::Other {
                        message: format!("Failed to parse YAML frontmatter: {}", e),
                    });
                }
            }
        };

        Ok(FrontmatterResult {
            metadata,
            content: remaining_content.to_string(),
        })
    } else {
        // No closing delimiter found, treat as no frontmatter
        Ok(FrontmatterResult {
            metadata: None,
            content: content.to_string(),
        })
    }
}

/// Find the closing frontmatter delimiter ("---" on its own line)
fn find_closing_delimiter(content: &str) -> Option<usize> {
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "---" {
            // Calculate the byte position of this line
            let mut pos = 0;
            for line in lines.iter().take(i) {
                pos += line.len();
                pos += 1; // for the newline character
            }
            return Some(pos);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_with_yaml() {
        let content = r#"---
name: test-mode
description: A test mode
---
You are a test agent.
This is the system prompt.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());

        let metadata = result.metadata.unwrap();
        assert_eq!(
            metadata.get("name").and_then(|v| v.as_str()),
            Some("test-mode")
        );
        assert_eq!(
            metadata.get("description").and_then(|v| v.as_str()),
            Some("A test mode")
        );

        assert!(result.content.starts_with("You are a test agent."));
    }

    #[test]
    fn test_parse_frontmatter_no_yaml() {
        let content = "You are an agent.\nThis is just content.";

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_empty_yaml() {
        let content = r#"---
---
System prompt here
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content.trim(), "System prompt here");
    }

    #[test]
    fn test_parse_frontmatter_malformed() {
        let content = r#"---
invalid yaml: [
---
Content
"#;

        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_frontmatter_no_closing_delimiter() {
        let content = r#"---
name: test
description: test
Content without closing delimiter
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }
}
