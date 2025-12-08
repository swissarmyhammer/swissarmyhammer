//! YAML frontmatter generation and manipulation utilities.
//!
//! This module provides tools for creating, parsing, and manipulating YAML frontmatter
//! in markdown documents. It includes a builder pattern for constructing frontmatter
//! and helper functions for combining frontmatter with content.
//!
//! # Usage Examples
//!
//! ## Basic Frontmatter Building
//!
//! ```rust
//! use markdowndown::frontmatter::FrontmatterBuilder;
//! use chrono::Utc;
//!
//! let frontmatter = FrontmatterBuilder::new("https://example.com".to_string())
//!     .exporter("markdowndown".to_string())
//!     .download_date(Utc::now())
//!     .additional_field("title".to_string(), "My Document".to_string())
//!     .build()?;
//!
//! println!("Generated frontmatter:\n{}", frontmatter);
//! # Ok::<(), markdowndown::types::MarkdownError>(())
//! ```
//!
//! ## Combining Frontmatter with Content
//!
//! ```rust
//! use markdowndown::frontmatter::{FrontmatterBuilder, combine_frontmatter_and_content};
//! use chrono::Utc;
//!
//! let frontmatter = FrontmatterBuilder::new("https://example.com".to_string())
//!     .exporter("markdowndown".to_string())
//!     .download_date(Utc::now())
//!     .build()?;
//!
//! let content = "# My Document\n\nThis is the content.";
//! let complete_doc = combine_frontmatter_and_content(&frontmatter, content);
//!
//! println!("Complete document:\n{}", complete_doc);
//! # Ok::<(), markdowndown::types::MarkdownError>(())
//! ```

use crate::types::{Frontmatter, MarkdownError, Url};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Builder for constructing YAML frontmatter with validation and flexibility.
///
/// This builder provides a fluent interface for creating frontmatter with required
/// and optional fields, automatic validation, and proper YAML formatting.
#[derive(Debug, Clone)]
pub struct FrontmatterBuilder {
    source_url: String,
    exporter: Option<String>,
    download_date: Option<DateTime<Utc>>,
    additional_fields: HashMap<String, String>,
}

impl FrontmatterBuilder {
    /// Creates a new FrontmatterBuilder with the required source URL.
    ///
    /// # Arguments
    ///
    /// * `source_url` - The source URL of the document (must be valid HTTP/HTTPS URL)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::frontmatter::FrontmatterBuilder;
    ///
    /// let builder = FrontmatterBuilder::new("https://example.com".to_string());
    /// ```
    pub fn new(source_url: String) -> Self {
        Self {
            source_url,
            exporter: None,
            download_date: None,
            additional_fields: HashMap::new(),
        }
    }

    /// Sets the exporter/processor name.
    ///
    /// # Arguments
    ///
    /// * `exporter` - Name of the tool or process that generated this markdown
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::frontmatter::FrontmatterBuilder;
    ///
    /// let builder = FrontmatterBuilder::new("https://example.com".to_string())
    ///     .exporter("markdowndown-v1.0".to_string());
    /// ```
    pub fn exporter(mut self, exporter: String) -> Self {
        self.exporter = Some(exporter);
        self
    }

    /// Sets the download timestamp.
    ///
    /// # Arguments
    ///
    /// * `date` - The date and time when the document was downloaded
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::frontmatter::FrontmatterBuilder;
    /// use chrono::Utc;
    ///
    /// let builder = FrontmatterBuilder::new("https://example.com".to_string())
    ///     .download_date(Utc::now());
    /// ```
    pub fn download_date(mut self, date: DateTime<Utc>) -> Self {
        self.download_date = Some(date);
        self
    }

    /// Adds a custom field to the frontmatter.
    ///
    /// # Arguments
    ///
    /// * `key` - The field name
    /// * `value` - The field value
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::frontmatter::FrontmatterBuilder;
    ///
    /// let builder = FrontmatterBuilder::new("https://example.com".to_string())
    ///     .additional_field("title".to_string(), "My Document".to_string())
    ///     .additional_field("author".to_string(), "John Doe".to_string());
    /// ```
    pub fn additional_field(mut self, key: String, value: String) -> Self {
        self.additional_fields.insert(key, value);
        self
    }

    /// Builds the YAML frontmatter string.
    ///
    /// This method validates the source URL, creates a Frontmatter struct, and serializes
    /// it to YAML format with proper delimiters.
    ///
    /// # Returns
    ///
    /// A `Result` containing the formatted YAML frontmatter string or a `MarkdownError`.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::InvalidUrl` - If the source URL is not valid
    /// * `MarkdownError::ParseError` - If YAML serialization fails
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::frontmatter::FrontmatterBuilder;
    /// use chrono::Utc;
    ///
    /// let frontmatter = FrontmatterBuilder::new("https://example.com".to_string())
    ///     .exporter("markdowndown".to_string())
    ///     .download_date(Utc::now())
    ///     .build()?;
    ///
    /// assert!(frontmatter.starts_with("---\n"));
    /// assert!(frontmatter.ends_with("---\n"));
    /// # Ok::<(), markdowndown::types::MarkdownError>(())
    /// ```
    pub fn build(self) -> Result<String, MarkdownError> {
        // Store values for error messages before they get moved
        let source_url_str = self.source_url.clone();
        let additional_fields_count = self.additional_fields.len();

        // Validate and create URL
        let url = Url::new(self.source_url)?;

        // Create Frontmatter struct with defaults if not provided
        let frontmatter = Frontmatter {
            source_url: url,
            exporter: self.exporter.unwrap_or_else(|| "markdowndown".to_string()),
            date_downloaded: self.download_date.unwrap_or_else(Utc::now),
        };

        // Serialize to YAML
        let mut yaml_content =
            serde_yaml::to_string(&frontmatter).map_err(|e| MarkdownError::ParseError {
                message: format!(
                    "Failed to serialize frontmatter to YAML (source URL: {source_url_str}, {additional_fields_count} additional fields): {e}"
                ),
            })?;

        // Add additional fields if any
        if !self.additional_fields.is_empty() {
            // Parse the existing YAML to add additional fields
            let mut yaml_value: serde_yaml::Value =
                serde_yaml::from_str(&yaml_content).map_err(|e| MarkdownError::ParseError {
                    message: format!(
                        "Failed to parse generated YAML (content length: {} chars): {}",
                        yaml_content.len(),
                        e
                    ),
                })?;

            if let serde_yaml::Value::Mapping(ref mut map) = yaml_value {
                for (key, value) in self.additional_fields {
                    map.insert(
                        serde_yaml::Value::String(key),
                        serde_yaml::Value::String(value),
                    );
                }
            }

            yaml_content =
                serde_yaml::to_string(&yaml_value).map_err(|e| MarkdownError::ParseError {
                    message: format!(
                        "Failed to serialize extended frontmatter to YAML ({additional_fields_count} additional fields added): {e}"
                    ),
                })?;
        }

        // Format with YAML delimiters
        Ok(format!("---\n{yaml_content}---\n"))
    }
}

/// Combines YAML frontmatter with markdown content to create a complete document.
///
/// # Arguments
///
/// * `frontmatter` - The YAML frontmatter string (should include delimiters)
/// * `content` - The markdown content
///
/// # Returns
///
/// A complete markdown document with frontmatter header
///
/// # Examples
///
/// ```rust
/// use markdowndown::frontmatter::combine_frontmatter_and_content;
///
/// let frontmatter = "---\nsource_url: \"https://example.com\"\nexporter: \"markdowndown\"\n---\n";
/// let content = "# My Document\n\nThis is the content.";
/// let complete_doc = combine_frontmatter_and_content(frontmatter, content);
///
/// assert!(complete_doc.contains("---"));
/// assert!(complete_doc.contains("# My Document"));
/// ```
pub fn combine_frontmatter_and_content(frontmatter: &str, content: &str) -> String {
    format!("{frontmatter}\n{content}")
}

/// Extracts frontmatter from a markdown document.
///
/// This function parses a markdown document and extracts the YAML frontmatter
/// if present, returning it as a Frontmatter struct.
///
/// # Arguments
///
/// * `markdown` - The complete markdown document potentially containing frontmatter
///
/// # Returns
///
/// An `Option<Frontmatter>` containing the parsed frontmatter, or `None` if no valid
/// frontmatter is found.
///
/// # Examples
///
/// ```rust
/// use markdowndown::frontmatter::extract_frontmatter;
///
/// let markdown = "---\nsource_url: \"https://example.com\"\nexporter: \"markdowndown\"\ndate_downloaded: \"2023-01-01T00:00:00Z\"\n---\n\n# Content";
/// let frontmatter = extract_frontmatter(markdown);
/// assert!(frontmatter.is_some());
/// ```
pub fn extract_frontmatter(markdown: &str) -> Option<Frontmatter> {
    // Check if markdown starts with frontmatter delimiters
    if !markdown.starts_with("---\n") {
        return None;
    }

    // Find the closing delimiter
    let content_after_start = &markdown[4..]; // Skip "---\n"
    if let Some(end_pos) = content_after_start.find("\n---\n") {
        let yaml_content = &content_after_start[..end_pos];

        // Try to parse the YAML content
        serde_yaml::from_str(yaml_content).ok()
    } else {
        None
    }
}

/// Strips frontmatter from a markdown document, returning only the content.
///
/// # Arguments
///
/// * `markdown` - The complete markdown document potentially containing frontmatter
///
/// # Returns
///
/// The markdown content without frontmatter
///
/// # Examples
///
/// ```rust
/// use markdowndown::frontmatter::strip_frontmatter;
///
/// let markdown = "---\nsource_url: \"https://example.com\"\n---\n\n# My Title\n\nContent here.";
/// let content_only = strip_frontmatter(markdown);
/// assert_eq!(content_only, "# My Title\n\nContent here.");
/// ```
pub fn strip_frontmatter(markdown: &str) -> String {
    // Check if markdown starts with frontmatter delimiters
    if !markdown.starts_with("---\n") {
        return markdown.to_string();
    }

    // Find the closing delimiter
    let content_after_start = &markdown[4..]; // Skip "---\n"
    if let Some(end_pos) = content_after_start.find("\n---\n") {
        let content_start = 4 + end_pos + 5; // Skip "---\n" + content + "\n---\n"
        if content_start < markdown.len() {
            // Skip any leading newlines from the combination
            let remaining = &markdown[content_start..];
            if let Some(stripped) = remaining.strip_prefix('\n') {
                stripped.to_string()
            } else {
                remaining.to_string()
            }
        } else {
            String::new()
        }
    } else {
        // No closing delimiter found, return original
        markdown.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ValidationErrorKind;
    use chrono::DateTime;

    #[test]
    fn test_frontmatter_builder_new() {
        let builder = FrontmatterBuilder::new("https://example.com".to_string());
        assert_eq!(builder.source_url, "https://example.com");
        assert!(builder.exporter.is_none());
        assert!(builder.download_date.is_none());
        assert!(builder.additional_fields.is_empty());
    }

    #[test]
    fn test_frontmatter_builder_fluent_interface() {
        let date = DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let builder = FrontmatterBuilder::new("https://example.com".to_string())
            .exporter("test-exporter".to_string())
            .download_date(date)
            .additional_field("title".to_string(), "Test Document".to_string());

        assert_eq!(builder.source_url, "https://example.com");
        assert_eq!(builder.exporter, Some("test-exporter".to_string()));
        assert_eq!(builder.download_date, Some(date));
        assert_eq!(
            builder.additional_fields.get("title"),
            Some(&"Test Document".to_string())
        );
    }

    #[test]
    fn test_frontmatter_builder_build_basic() {
        let result = FrontmatterBuilder::new("https://example.com".to_string())
            .exporter("test-exporter".to_string())
            .build();

        assert!(result.is_ok());
        let frontmatter = result.unwrap();
        assert!(frontmatter.starts_with("---\n"));
        assert!(frontmatter.ends_with("---\n"));
        assert!(frontmatter.contains("source_url: https://example.com"));
        assert!(frontmatter.contains("exporter: test-exporter"));
        assert!(frontmatter.contains("date_downloaded:"));
    }

    #[test]
    fn test_frontmatter_builder_build_with_additional_fields() {
        let result = FrontmatterBuilder::new("https://example.com".to_string())
            .exporter("test-exporter".to_string())
            .additional_field("title".to_string(), "My Document".to_string())
            .additional_field("author".to_string(), "John Doe".to_string())
            .build();

        assert!(result.is_ok());
        let frontmatter = result.unwrap();
        assert!(frontmatter.contains("title: My Document"));
        assert!(frontmatter.contains("author: John Doe"));
    }

    #[test]
    fn test_frontmatter_builder_build_invalid_url() {
        let result = FrontmatterBuilder::new("not-a-url".to_string()).build();

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ValidationError { kind, context } => {
                assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                assert_eq!(context.url, "not-a-url");
            }
            _ => panic!("Expected ValidationError with InvalidUrl kind"),
        }
    }

    #[test]
    fn test_combine_frontmatter_and_content() {
        let frontmatter = "---\nsource_url: \"https://example.com\"\n---\n";
        let content = "# My Document\n\nThis is content.";
        let result = combine_frontmatter_and_content(frontmatter, content);

        assert_eq!(
            result,
            "---\nsource_url: \"https://example.com\"\n---\n\n# My Document\n\nThis is content."
        );
    }

    #[test]
    fn test_extract_frontmatter_valid() {
        let markdown = "---\nsource_url: https://example.com\nexporter: markdowndown\ndate_downloaded: \"2023-01-01T00:00:00Z\"\n---\n\n# Content";
        let result = extract_frontmatter(markdown);

        assert!(result.is_some());
        let frontmatter = result.unwrap();
        assert_eq!(frontmatter.source_url.as_str(), "https://example.com");
        assert_eq!(frontmatter.exporter, "markdowndown");
    }

    #[test]
    fn test_extract_frontmatter_no_frontmatter() {
        let markdown = "# Just Content\n\nNo frontmatter here.";
        let result = extract_frontmatter(markdown);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_frontmatter_incomplete() {
        let markdown = "---\nsource_url: \"https://example.com\"\nNo closing delimiter";
        let result = extract_frontmatter(markdown);
        assert!(result.is_none());
    }

    #[test]
    fn test_strip_frontmatter_with_frontmatter() {
        let markdown = "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n\n# My Title\n\nContent here.";
        let result = strip_frontmatter(markdown);
        assert_eq!(result, "# My Title\n\nContent here.");
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let markdown = "# My Title\n\nContent here.";
        let result = strip_frontmatter(markdown);
        assert_eq!(result, "# My Title\n\nContent here.");
    }

    #[test]
    fn test_strip_frontmatter_incomplete() {
        let markdown = "---\nsource_url: \"https://example.com\"\nNo closing delimiter\n# Content";
        let result = strip_frontmatter(markdown);
        assert_eq!(result, markdown); // Should return original if incomplete
    }

    #[test]
    fn test_roundtrip_frontmatter_extraction() {
        // Test that we can build frontmatter, combine with content, then extract it back
        let original_date = DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let frontmatter_str = FrontmatterBuilder::new("https://example.com".to_string())
            .exporter("test-exporter".to_string())
            .download_date(original_date)
            .build()
            .unwrap();

        let content = "# Test Document\n\nThis is test content.";
        let complete_doc = combine_frontmatter_and_content(&frontmatter_str, content);

        // Extract frontmatter back
        let extracted = extract_frontmatter(&complete_doc);
        assert!(extracted.is_some());

        let extracted_frontmatter = extracted.unwrap();
        assert_eq!(
            extracted_frontmatter.source_url.as_str(),
            "https://example.com"
        );
        assert_eq!(extracted_frontmatter.exporter, "test-exporter");
        assert_eq!(extracted_frontmatter.date_downloaded, original_date);

        // Extract content back
        let extracted_content = strip_frontmatter(&complete_doc);
        assert_eq!(extracted_content, content);
    }
}
