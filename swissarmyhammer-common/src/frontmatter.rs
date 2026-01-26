//! Shared frontmatter parsing functionality
//!
//! This module provides common frontmatter parsing logic used by both
//! workflow and prompt parsers to eliminate code duplication.
//!
//! # YAML Include Expansion
//!
//! Frontmatter supports `@path/to/file` references that expand to the contents
//! of YAML files loaded from the standard directory hierarchy. Use
//! `parse_frontmatter_with_expansion` to enable this feature.
//!
//! ## Example
//!
//! Given `file_groups/source_code.yaml`:
//! ```yaml
//! - "*.js"
//! - "*.ts"
//! ```
//!
//! You can reference it in frontmatter:
//! ```yaml
//! ---
//! match:
//!   files:
//!     - "@file_groups/source_code"
//!     - "*.custom"
//! ---
//! ```

use crate::{Result, SwissArmyHammerError};
use swissarmyhammer_directory::{DirectoryConfig, YamlExpander};

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
/// use swissarmyhammer_common::frontmatter::parse_frontmatter;
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
    parse_frontmatter_internal(
        content,
        None::<&YamlExpander<swissarmyhammer_directory::SwissarmyhammerConfig>>,
    )
}

/// Parses YAML frontmatter with `@` include expansion.
///
/// This is like `parse_frontmatter` but expands `@path/to/file` references
/// in the YAML using the provided expander.
///
/// # Arguments
/// * `content` - The raw content potentially containing YAML frontmatter
/// * `expander` - The YAML expander with loaded includes
///
/// # Examples
/// ```ignore
/// use swissarmyhammer_common::frontmatter::parse_frontmatter_with_expansion;
/// use swissarmyhammer_directory::{YamlExpander, SwissarmyhammerConfig};
///
/// let mut expander = YamlExpander::<SwissarmyhammerConfig>::new();
/// expander.load_all().unwrap();
///
/// let content = r#"---
/// files:
///   - "@file_groups/source_code"
/// ---
/// Content here
/// "#;
///
/// let result = parse_frontmatter_with_expansion(content, &expander).unwrap();
/// ```
pub fn parse_frontmatter_with_expansion<C: DirectoryConfig>(
    content: &str,
    expander: &YamlExpander<C>,
) -> Result<Frontmatter> {
    parse_frontmatter_internal(content, Some(expander))
}

/// Internal implementation that optionally expands includes.
fn parse_frontmatter_internal<C: DirectoryConfig>(
    content: &str,
    expander: Option<&YamlExpander<C>>,
) -> Result<Frontmatter> {
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
            let mut yaml_value: serde_yaml::Value =
                serde_yaml::from_str(yaml_content).map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Invalid YAML frontmatter: {e}"),
                })?;

            // Expand includes if an expander is provided
            if let Some(exp) = expander {
                yaml_value = exp
                    .expand(yaml_value)
                    .map_err(|e| SwissArmyHammerError::Other {
                        message: format!("Failed to expand YAML includes: {e}"),
                    })?;
            }

            // Convert to JSON for consistent handling
            let json_value =
                serde_json::to_value(yaml_value).map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to convert YAML to JSON: {e}"),
                })?;

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
