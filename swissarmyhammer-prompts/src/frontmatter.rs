//! YAML frontmatter parsing for prompt files
//!
//! This module provides functionality to parse YAML frontmatter from
//! markdown files containing prompts.

use crate::Result;
use serde_json::Value;
use swissarmyhammer_common::SwissArmyHammerError;

/// Represents parsed frontmatter and content
#[derive(Debug, Clone)]
pub struct FrontmatterResult {
    /// Parsed YAML metadata (None if no frontmatter)
    pub metadata: Option<Value>,
    /// Remaining content after frontmatter removal
    pub content: String,
}

/// Parse YAML frontmatter from content
///
/// Expects frontmatter to be delimited by `---` at the beginning and end,
/// followed by the main content. Returns the parsed metadata and remaining content.
///
/// # Format
/// ```markdown
/// ---
/// name: example
/// description: An example prompt
/// ---
/// Main content goes here
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
    } else if after_first_delimiter.starts_with('\r') {
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
            for j in 0..i {
                pos += lines[j].len();
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
name: test_prompt
description: A test prompt
tags:
  - test
  - example
---
Hello {{name}}!
This is the template content.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());
        
        let metadata = result.metadata.unwrap();
        assert_eq!(
            metadata.get("name").and_then(|v| v.as_str()),
            Some("test_prompt")
        );
        assert_eq!(
            metadata.get("description").and_then(|v| v.as_str()),
            Some("A test prompt")
        );
        
        let tags: Vec<String> = metadata
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(tags, vec!["test", "example"]);
        
        assert!(result.content.starts_with("Hello {{name}}!"));
    }

    #[test]
    fn test_parse_frontmatter_no_yaml() {
        let content = "Hello {{name}}!\nThis is just content.";
        
        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_empty_yaml() {
        let content = r#"---
---
Content here
"#;
        
        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content.trim(), "Content here");
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

    #[test]
    fn test_find_closing_delimiter() {
        let content = "line1\nline2\n---\nline4\n";
        let pos = find_closing_delimiter(content);
        assert!(pos.is_some());
        
        let content_no_delimiter = "line1\nline2\nline3\n";
        let pos = find_closing_delimiter(content_no_delimiter);
        assert!(pos.is_none());
    }

    #[test]
    fn test_yaml_field_access() {
        let yaml = serde_yaml::from_str::<Value>(r#"
name: test
count: 42
enabled: true
tags: [a, b, c]
"#).unwrap();

        assert_eq!(yaml.get("name").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(yaml.get("count").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(yaml.get("enabled").and_then(|v| v.as_bool()), Some(true));
        
        let tags: Vec<String> = yaml
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(tags, vec!["a", "b", "c"]);
    }
}