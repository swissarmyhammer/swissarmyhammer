//! Shared frontmatter parsing functionality
//!
//! This module provides common frontmatter parsing logic used by both
//! workflow and prompt parsers to eliminate code duplication.

use crate::{Result, SwissArmyHammerError};

/// Represents parsed frontmatter with metadata and content
#[derive(Debug, Clone)]
pub struct Frontmatter {
    /// Parsed YAML metadata as a serde_json::Value, or None if no frontmatter
    pub metadata: Option<serde_json::Value>,
    /// The content after the frontmatter (or entire content if no frontmatter)
    pub content: String,
}

/// Parses YAML frontmatter from markdown content
///
/// Handles content with YAML frontmatter delimited by `---` markers.
/// If no frontmatter is found, returns the entire content unchanged.
///
/// # Arguments
/// * `content` - The raw content potentially containing YAML frontmatter
///
/// # Returns
/// * `Ok(Frontmatter)` - Successfully parsed frontmatter and content
/// * `Err(_)` - YAML parsing error if frontmatter is malformed
///
/// # Examples
/// ```
/// use swissarmyhammer::frontmatter::parse_frontmatter;
///
/// let content = r#"---
/// title: Example
/// description: A test document
/// ---
///
/// Main Content
/// This is the body.
/// "#;
///
/// let result = parse_frontmatter(content).unwrap();
/// assert!(result.metadata.is_some());
/// assert!(result.content.contains("Main Content"));
/// ```
pub fn parse_frontmatter(content: &str) -> Result<Frontmatter> {
    // Check for partial marker first - these don't have frontmatter
    if content.trim_start().starts_with("{% partial %}") {
        return Ok(Frontmatter {
            metadata: None,
            content: content.to_string(),
        });
    }

    // Check for YAML frontmatter delimiter
    if content.starts_with("---\n") {
        let parts: Vec<&str> = content.splitn(3, "---\n").collect();
        if parts.len() >= 3 {
            let yaml_content = parts[1];
            let body_content = parts[2].to_string();

            // Parse YAML frontmatter
            let yaml_value: serde_yaml::Value =
                serde_yaml::from_str(yaml_content).map_err(|e| {
                    SwissArmyHammerError::Other(format!("Invalid YAML frontmatter: {e}"))
                })?;

            // Convert to JSON for consistent handling
            let json_value = serde_json::to_value(yaml_value)
                .map_err(|e| SwissArmyHammerError::Other(e.to_string()))?;

            return Ok(Frontmatter {
                metadata: Some(json_value),
                content: body_content,
            });
        }
    }

    // No frontmatter found, return entire content
    Ok(Frontmatter {
        metadata: None,
        content: content.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_with_yaml() {
        let content = r#"---
title: Test Document
description: A test document
parameters:
  - name: test_param
    required: true
---

# Main Content
This is the body content.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());

        let metadata = result.metadata.as_ref().unwrap();
        assert_eq!(
            metadata.get("title").and_then(|v| v.as_str()),
            Some("Test Document")
        );
        assert_eq!(
            metadata.get("description").and_then(|v| v.as_str()),
            Some("A test document")
        );
        assert!(metadata.get("parameters").is_some());

        assert!(result.content.contains("# Main Content"));
        assert!(result.content.contains("This is the body content."));
        assert!(!result.content.starts_with("---"));
    }

    #[test]
    fn test_parse_frontmatter_without_yaml() {
        let content = r#"# Just Regular Content
This is just regular markdown without frontmatter.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_with_partial_marker() {
        let content = r#"{% partial %}
<div class="header">
  <h1>{{title}}</h1>
</div>"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_malformed_yaml() {
        let content = r#"---
title: Test
invalid_yaml: [unclosed
---

Content here
"#;

        let result = parse_frontmatter(content);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid YAML frontmatter"));
    }

    #[test]
    fn test_parse_frontmatter_incomplete_delimiter() {
        let content = r#"---
title: Test
description: Missing closing delimiter

Content here
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_none());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_frontmatter_empty_yaml() {
        let content = r#"---
---

Content after empty frontmatter
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());
        let metadata = result.metadata.as_ref().unwrap();
        assert!(metadata.is_null());
        assert!(result.content.contains("Content after empty frontmatter"));
    }

    #[test]
    fn test_parse_frontmatter_content_preservation() {
        let content = r#"---
title: Test
---

    # Content with Leading Whitespace
This should have leading whitespace preserved.
"#;

        let result = parse_frontmatter(content).unwrap();
        assert!(result.metadata.is_some());

        // Content should preserve ALL whitespace after frontmatter, including newlines and indentation
        assert!(result
            .content
            .starts_with("\n    # Content with Leading Whitespace"));
        assert!(result
            .content
            .contains("This should have leading whitespace"));
    }
}
