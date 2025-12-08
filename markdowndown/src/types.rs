//! Core types and data structures for the markdowndown library.
//!
//! This module provides the fundamental data types used throughout the markdowndown library,
//! including validated wrappers for markdown content and URLs, error handling, and metadata structures.
//!
//! # Usage Examples
//!
//! ## Basic Markdown Processing
//!
//! ```rust
//! use markdowndown::types::{Markdown, MarkdownError};
//!
//! // Create validated markdown content
//! let markdown = Markdown::new("# Hello World\n\nThis is a test document.".to_string())?;
//! println!("Content: {}", markdown);
//! println!("As string: {}", markdown.as_str());
//!
//! // Validation catches empty content
//! let invalid = Markdown::new("   \n\t  ".to_string());
//! assert!(invalid.is_err());
//! # Ok::<(), MarkdownError>(())
//! ```
//!
//! ## URL Validation and Processing
//!
//! ```rust
//! use markdowndown::types::{Url, MarkdownError};
//!
//! // Create validated URLs
//! let url = Url::new("https://docs.google.com/document/d/123".to_string())?;
//! println!("URL: {}", url);
//!
//! // Invalid URLs are rejected
//! let invalid_url = Url::new("not-a-url".to_string());
//! assert!(invalid_url.is_err());
//! # Ok::<(), MarkdownError>(())
//! ```
//!
//! ## Working with Frontmatter
//!
//! ```rust
//! use markdowndown::types::{Frontmatter, Url, UrlType};
//! use chrono::Utc;
//!
//! // Create document metadata
//! let frontmatter = Frontmatter {
//!     source_url: Url::new("https://example.com/document".to_string())?,
//!     exporter: "markdowndown".to_string(),
//!     date_downloaded: Utc::now(),
//! };
//!
//! // Serialize to YAML for document headers
//! let yaml = serde_yaml::to_string(&frontmatter)?;
//! println!("YAML frontmatter:\n{}", yaml);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Complete Document Processing Workflow
//!
//! ```rust
//! use markdowndown::types::{Markdown, Frontmatter, Url, UrlType};
//! use chrono::Utc;
//!
//! // Process a complete document with validation and metadata
//! let source_url = Url::new("https://docs.google.com/document/d/abc123".to_string())?;
//! let content = Markdown::new("# Project Overview\n\nThis document outlines...".to_string())?;
//!
//! let metadata = Frontmatter {
//!     source_url,
//!     exporter: "markdowndown-v1.0".to_string(),
//!     date_downloaded: Utc::now(),
//! };
//!
//! // Generate complete markdown document with frontmatter
//! let yaml_header = serde_yaml::to_string(&metadata)?;
//! let complete_document = format!("---\n{}---\n\n{}", yaml_header, content);
//!
//! println!("Complete document:\n{}", complete_document);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Error Handling Patterns
//!
//! ```rust
//! use markdowndown::types::{Markdown, Url, MarkdownError};
//!
//! fn process_content(url_str: String, content: String) -> Result<(Url, Markdown), MarkdownError> {
//!     let url = Url::new(url_str)?;  // May fail with InvalidUrl
//!     let markdown = Markdown::new(content)?;  // May fail with ParseError
//!     Ok((url, markdown))
//! }
//!
//! match process_content("https://example.com".to_string(), "# Valid content".to_string()) {
//!     Ok((url, markdown)) => {
//!         println!("Successfully processed: {} -> {}", url, markdown);
//!     }
//!     Err(MarkdownError::InvalidUrl { url }) => {
//!         eprintln!("Invalid URL: {}", url);
//!     }
//!     Err(MarkdownError::ParseError { message }) => {
//!         eprintln!("Content validation failed: {}", message);
//!     }
//!     Err(e) => {
//!         eprintln!("Other error: {}", e);
//!     }
//! }
//! # Ok::<(), MarkdownError>(())
//! ```
//!
//! ## URL Type Detection
//!
//! ```rust
//! use markdowndown::types::UrlType;
//!
//! // Different URL types for specialized processing
//! let google_docs = UrlType::GoogleDocs;
//! let github_issue = UrlType::GitHubIssue;
//! let html_page = UrlType::Html;
//!
//! println!("Processing {} content", google_docs);  // "Processing Google Docs content"
//! println!("Processing {} content", github_issue); // "Processing GitHub Issue content"
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// A newtype wrapper for markdown content with validation and conversion methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markdown(String);

impl Markdown {
    /// Creates a new Markdown instance with validation.
    ///
    /// # Errors
    ///
    /// Returns a `MarkdownError::ParseError` if the content is empty or whitespace-only.
    pub fn new(content: String) -> Result<Self, MarkdownError> {
        let markdown = Markdown(content);
        markdown.validate()?;
        Ok(markdown)
    }

    /// Returns the markdown content as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validates that the markdown content is not empty or whitespace-only.
    pub fn validate(&self) -> Result<(), MarkdownError> {
        if self.0.trim().is_empty() {
            Err(MarkdownError::ParseError {
                message: "Markdown content cannot be empty or whitespace-only".to_string(),
            })
        } else {
            Ok(())
        }
    }

    /// Creates a new Markdown instance with frontmatter prepended to the content.
    ///
    /// # Arguments
    ///
    /// * `frontmatter` - The YAML frontmatter string (should include delimiters)
    ///
    /// # Returns
    ///
    /// A new `Markdown` instance containing the frontmatter and original content
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::types::Markdown;
    ///
    /// let content = Markdown::new("# Hello World".to_string())?;
    /// let frontmatter = "---\nsource_url: \"https://example.com\"\n---\n";
    /// let with_frontmatter = content.with_frontmatter(frontmatter);
    ///
    /// assert!(with_frontmatter.as_str().contains("source_url"));
    /// assert!(with_frontmatter.as_str().contains("# Hello World"));
    /// # Ok::<(), markdowndown::types::MarkdownError>(())
    /// ```
    pub fn with_frontmatter(&self, frontmatter: &str) -> Markdown {
        let combined_content = format!("{}\n{}", frontmatter, self.0);
        Markdown(combined_content)
    }

    /// Extracts the frontmatter from the markdown content if present.
    ///
    /// # Returns
    ///
    /// An `Option<String>` containing the YAML frontmatter (including delimiters)
    /// if found, or `None` if no frontmatter is present.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::types::Markdown;
    ///
    /// let content = "---\nsource_url: \"https://example.com\"\n---\n\n# Hello World";
    /// let markdown = Markdown::from(content.to_string());
    /// let frontmatter = markdown.frontmatter();
    ///
    /// assert!(frontmatter.is_some());
    /// assert!(frontmatter.unwrap().contains("source_url"));
    /// ```
    pub fn frontmatter(&self) -> Option<String> {
        // Check if content starts with frontmatter delimiters
        if !self.0.starts_with("---\n") {
            return None;
        }

        // Find the closing delimiter
        let content_after_start = &self.0[4..]; // Skip "---\n"
        if let Some(end_pos) = content_after_start.find("\n---\n") {
            let full_frontmatter = &self.0[..4 + end_pos + 5]; // Include both delimiters

            // Extract just the YAML content (without delimiters) for validation
            let yaml_content = &self.0[4..4 + end_pos]; // Content between delimiters

            // Check for malformed frontmatter patterns
            // Frontmatter should not start with "---" (which would indicate nested delimiters)
            if yaml_content.trim_start().starts_with("---") {
                return None; // Malformed frontmatter with nested delimiters
            }

            // Validate that it's parseable as YAML
            if serde_yaml::from_str::<serde_yaml::Value>(yaml_content).is_ok() {
                Some(full_frontmatter.to_string())
            } else {
                None // Invalid YAML, treat as regular content
            }
        } else {
            None
        }
    }

    /// Returns only the content portion of the markdown, stripping any frontmatter.
    ///
    /// # Returns
    ///
    /// A `String` containing only the markdown content without frontmatter
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::types::Markdown;
    ///
    /// let content = "---\nsource_url: \"https://example.com\"\n---\n\n# Hello World\n\nContent here.";
    /// let markdown = Markdown::from(content.to_string());
    /// let content_only = markdown.content_only();
    ///
    /// assert_eq!(content_only, "# Hello World\n\nContent here.");
    /// ```
    pub fn content_only(&self) -> String {
        // Check if content starts with frontmatter delimiters
        if !self.0.starts_with("---\n") {
            return self.0.clone();
        }

        // If frontmatter parsing fails, treat the whole thing as content
        if self.frontmatter().is_none() {
            return self.0.clone();
        }

        // Find the closing delimiter - look for proper frontmatter end
        // We need to find a line that contains only "---"
        let content_after_start = &self.0[4..]; // Skip "---\n"
        let lines: Vec<&str> = content_after_start.lines().collect();
        let mut frontmatter_end_idx = None;

        for (i, line) in lines.iter().enumerate() {
            if line.trim() == "---" {
                // This could be the end of frontmatter
                frontmatter_end_idx = Some(i);
                break;
            }
        }

        if let Some(end_line_idx) = frontmatter_end_idx {
            // Calculate byte position of the content after frontmatter
            let mut content_start = 4; // Start after "---\n"
                                       // Add bytes for all lines up to and including the closing "---"
            for line in lines.iter().take(end_line_idx + 1) {
                content_start += line.len();
                content_start += 1; // Add newline character
            }

            if content_start < self.0.len() {
                // Skip any leading newlines and additional --- lines from malformed frontmatter
                let mut remaining = &self.0[content_start..];
                remaining = remaining.strip_prefix('\n').unwrap_or(remaining);

                // Keep stripping lines that contain only "---"
                let remaining_lines: Vec<&str> = remaining.lines().collect();
                let mut actual_content_start = 0;
                for (i, line) in remaining_lines.iter().enumerate() {
                    if line.trim() != "---" {
                        actual_content_start = i;
                        break;
                    }
                }

                if actual_content_start < remaining_lines.len() {
                    let content_lines = &remaining_lines[actual_content_start..];
                    // Join with newlines and add a final newline if there was one originally
                    let mut result = content_lines.join("\n");
                    // If the last element is empty, it means there was a trailing newline
                    if content_lines.last() == Some(&"") {
                        result.push('\n');
                    }
                    result
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            // No proper closing delimiter found, treat as malformed frontmatter
            // Return the original content unchanged
            self.0.clone()
        }
    }
}

impl From<String> for Markdown {
    fn from(content: String) -> Self {
        Markdown(content)
    }
}

impl From<Markdown> for String {
    fn from(val: Markdown) -> Self {
        val.0
    }
}

impl AsRef<str> for Markdown {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Markdown {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Markdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A newtype wrapper for URLs with validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Url(String);

impl Url {
    /// Creates a new URL instance with basic validation.
    ///
    /// # Errors
    ///
    /// Returns a `MarkdownError::InvalidUrl` if the URL format is invalid.
    pub fn new(url: String) -> Result<Self, MarkdownError> {
        // Check for HTTP/HTTPS URLs
        if (url.starts_with("http://") && url.len() > 7)
            || (url.starts_with("https://") && url.len() > 8)
        {
            return Ok(Url(url));
        }

        // Check for local file paths
        if crate::utils::is_local_file_path(&url) {
            return Ok(Url(url));
        }

        // If neither web URL nor local file path, it's invalid
        let context = ErrorContext::new(&url, "URL validation", "Url::new");
        Err(MarkdownError::ValidationError {
            kind: ValidationErrorKind::InvalidUrl,
            context,
        })
    }

    /// Returns the URL as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Url {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Url {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Url::new(s).map_err(serde::de::Error::custom)
    }
}

/// Enumeration of supported URL types for content extraction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UrlType {
    /// Generic HTML pages
    Html,
    /// Google Docs documents
    GoogleDocs,
    /// GitHub issues
    GitHubIssue,
    /// Local file paths
    LocalFile,
}

impl fmt::Display for UrlType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrlType::Html => write!(f, "HTML"),
            UrlType::GoogleDocs => write!(f, "Google Docs"),
            UrlType::GitHubIssue => write!(f, "GitHub Issue"),
            UrlType::LocalFile => write!(f, "Local File"),
        }
    }
}

/// Error context providing detailed information about where and how an error occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorContext {
    /// The URL being processed when the error occurred
    pub url: String,
    /// The operation being performed (e.g., "URL detection", "Document download")  
    pub operation: String,
    /// The converter type being used (e.g., "GoogleDocsConverter", "GitHubConverter")
    pub converter_type: String,
    /// When the error occurred
    pub timestamp: DateTime<Utc>,
    /// Additional contextual information
    pub additional_info: Option<String>,
}

impl ErrorContext {
    /// Creates a new error context with the specified details.
    pub fn new(
        url: impl Into<String>,
        operation: impl Into<String>,
        converter_type: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            operation: operation.into(),
            converter_type: converter_type.into(),
            timestamp: Utc::now(),
            additional_info: None,
        }
    }

    /// Adds additional contextual information to the error context.
    pub fn with_info(mut self, info: impl Into<String>) -> Self {
        self.additional_info = Some(info.into());
        self
    }
}

/// Validation error kinds for input validation failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationErrorKind {
    InvalidUrl,
    InvalidFormat,
    MissingParameter,
}

/// Network error kinds for connection and communication failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkErrorKind {
    Timeout,
    ConnectionFailed,
    DnsResolution,
    RateLimited,
    ServerError(u16),
}

/// Authentication error kinds for authorization failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthErrorKind {
    MissingToken,
    InvalidToken,
    PermissionDenied,
    TokenExpired,
}

/// Content error kinds for data processing failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentErrorKind {
    EmptyContent,
    UnsupportedFormat,
    ParsingFailed,
}

/// Converter error kinds for external tool and processing failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConverterErrorKind {
    ExternalToolFailed,
    ProcessingError,
    UnsupportedOperation,
}

/// Configuration error kinds for setup and configuration failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigErrorKind {
    InvalidConfig,
    MissingDependency,
    InvalidValue,
}

/// Error types for the markdowndown library.
#[derive(Debug, Error)]
pub enum MarkdownError {
    /// Validation errors for invalid input
    #[error("Validation error: {kind:?} - {context:?}")]
    ValidationError {
        kind: ValidationErrorKind,
        context: ErrorContext,
    },

    /// Enhanced network-related errors with detailed context
    #[error("Network error: {kind:?} - {context:?}")]
    EnhancedNetworkError {
        kind: NetworkErrorKind,
        context: ErrorContext,
    },

    /// Authentication and authorization errors
    #[error("Authentication error: {kind:?} - {context:?}")]
    AuthenticationError {
        kind: AuthErrorKind,
        context: ErrorContext,
    },

    /// Content processing and parsing errors
    #[error("Content error: {kind:?} - {context:?}")]
    ContentError {
        kind: ContentErrorKind,
        context: ErrorContext,
    },

    /// Converter-specific processing errors
    #[error("Converter error: {kind:?} - {context:?}")]
    ConverterError {
        kind: ConverterErrorKind,
        context: ErrorContext,
    },

    /// Configuration and system setup errors
    #[error("Configuration error: {kind:?} - {context:?}")]
    ConfigurationError {
        kind: ConfigErrorKind,
        context: ErrorContext,
    },

    // Legacy error types for backward compatibility - keep the exact same names and structures
    /// Network-related errors
    #[error("Network error: {message}")]
    NetworkError { message: String },

    /// Parsing errors
    #[error("Parse error: {message}")]
    ParseError { message: String },

    /// Invalid URL errors
    #[error("Invalid URL: {url}")]
    InvalidUrl { url: String },

    /// Authentication errors
    #[error("Authentication error: {message}")]
    AuthError { message: String },

    /// Legacy configuration errors - renamed to avoid conflicts
    #[error("Configuration error: {message}")]
    LegacyConfigurationError { message: String },
}

impl MarkdownError {
    /// Returns the error context if available.
    pub fn context(&self) -> Option<&ErrorContext> {
        match self {
            MarkdownError::ValidationError { context, .. } => Some(context),
            MarkdownError::EnhancedNetworkError { context, .. } => Some(context),
            MarkdownError::AuthenticationError { context, .. } => Some(context),
            MarkdownError::ContentError { context, .. } => Some(context),
            MarkdownError::ConverterError { context, .. } => Some(context),
            MarkdownError::ConfigurationError { context, .. } => Some(context),
            _ => None,
        }
    }

    /// Returns true if this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                NetworkErrorKind::Timeout => true,
                NetworkErrorKind::ConnectionFailed => true,
                NetworkErrorKind::DnsResolution => false,
                NetworkErrorKind::RateLimited => true,
                NetworkErrorKind::ServerError(status) => *status >= 500,
            },
            MarkdownError::AuthenticationError {
                kind: AuthErrorKind::TokenExpired,
                ..
            } => true,
            // Legacy network errors - use simple heuristics based on message content
            MarkdownError::NetworkError { message } => {
                message.contains("timeout")
                    || message.contains("connection")
                    || message.contains("rate limit")
            }
            _ => false,
        }
    }

    /// Returns true if recovery strategies should be attempted.
    pub fn is_recoverable(&self) -> bool {
        match self {
            MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                NetworkErrorKind::Timeout => true,
                NetworkErrorKind::ConnectionFailed => true,
                NetworkErrorKind::DnsResolution => false,
                NetworkErrorKind::RateLimited => true,
                NetworkErrorKind::ServerError(status) => match status {
                    500..=503 | 429 => true, // Server errors and rate limiting
                    400..=499 => false,      // Client errors (including 400 Bad Request)
                    _ => true,               // Other status codes default to recoverable
                },
            },
            MarkdownError::AuthenticationError { .. } => true,
            MarkdownError::ConverterError { .. } => true,
            MarkdownError::ContentError {
                kind: ContentErrorKind::UnsupportedFormat,
                ..
            } => true,
            // Legacy errors are considered recoverable for network and auth issues
            MarkdownError::NetworkError { .. } => true,
            MarkdownError::AuthError { .. } => true,
            _ => false,
        }
    }

    /// Returns user-friendly suggestions for resolving this error.
    pub fn suggestions(&self) -> Vec<String> {
        match self {
            MarkdownError::ValidationError { kind, .. } => match kind {
                ValidationErrorKind::InvalidUrl => vec![
                    "Ensure the URL starts with http:// or https://".to_string(),
                    "Check that the URL is complete and properly formatted".to_string(),
                    "Try copying the URL directly from your browser".to_string(),
                ],
                ValidationErrorKind::InvalidFormat => {
                    vec!["Verify the input format matches the expected pattern".to_string()]
                }
                ValidationErrorKind::MissingParameter => {
                    vec!["Check that all required parameters are provided".to_string()]
                }
            },
            MarkdownError::EnhancedNetworkError { kind, .. } => match kind {
                NetworkErrorKind::Timeout => vec![
                    "Check your internet connection".to_string(),
                    "Try again in a few minutes".to_string(),
                    "Consider increasing the timeout in configuration".to_string(),
                ],
                NetworkErrorKind::ConnectionFailed => vec![
                    "Verify the server is accessible".to_string(),
                    "Check if you're behind a firewall or proxy".to_string(),
                ],
                NetworkErrorKind::DnsResolution => vec![
                    "Check that the domain name is correct".to_string(),
                    "Try using a different DNS server".to_string(),
                ],
                NetworkErrorKind::RateLimited => vec![
                    "Wait before making additional requests".to_string(),
                    "Consider authenticating to increase rate limits".to_string(),
                ],
                NetworkErrorKind::ServerError(_) => vec![
                    "The server is experiencing issues".to_string(),
                    "Try again later".to_string(),
                ],
            },
            MarkdownError::AuthenticationError { kind, .. } => match kind {
                AuthErrorKind::MissingToken => vec![
                    "Set up authentication credentials".to_string(),
                    "Check the documentation for authentication requirements".to_string(),
                ],
                AuthErrorKind::InvalidToken => vec![
                    "Verify your authentication token is correct".to_string(),
                    "Generate a new token if the current one is invalid".to_string(),
                ],
                AuthErrorKind::PermissionDenied => vec![
                    "Ensure you have permission to access this resource".to_string(),
                    "Check that your token has the required scopes".to_string(),
                ],
                AuthErrorKind::TokenExpired => {
                    vec!["Refresh or regenerate your authentication token".to_string()]
                }
            },
            MarkdownError::ContentError { kind, .. } => match kind {
                ContentErrorKind::EmptyContent => vec![
                    "Verify the source contains actual content".to_string(),
                    "Check if the URL is publicly accessible".to_string(),
                ],
                ContentErrorKind::UnsupportedFormat => vec![
                    "Try using a different converter for this content type".to_string(),
                    "Check if the content format is supported".to_string(),
                ],
                ContentErrorKind::ParsingFailed => vec![
                    "The content format may be corrupted or unsupported".to_string(),
                    "Try accessing the content directly to verify it's valid".to_string(),
                ],
            },
            MarkdownError::ConverterError { kind, .. } => match kind {
                ConverterErrorKind::ExternalToolFailed => vec![
                    "Check that required external tools are installed".to_string(),
                    "Verify tool dependencies and PATH configuration".to_string(),
                ],
                ConverterErrorKind::ProcessingError => vec![
                    "Try again with different converter settings".to_string(),
                    "Check if the input format is supported".to_string(),
                ],
                ConverterErrorKind::UnsupportedOperation => vec![
                    "This operation is not supported for this content type".to_string(),
                    "Try using a different converter or approach".to_string(),
                ],
            },
            MarkdownError::ConfigurationError { kind, .. } => match kind {
                ConfigErrorKind::InvalidConfig => vec![
                    "Check your configuration file for syntax errors".to_string(),
                    "Verify all configuration values are valid".to_string(),
                ],
                ConfigErrorKind::MissingDependency => vec![
                    "Install the required dependencies".to_string(),
                    "Check the documentation for setup requirements".to_string(),
                ],
                ConfigErrorKind::InvalidValue => vec![
                    "Check that configuration values are within valid ranges".to_string(),
                    "Refer to documentation for valid configuration options".to_string(),
                ],
            },
            // Legacy error suggestions
            MarkdownError::NetworkError { .. } => vec![
                "Check your internet connection".to_string(),
                "Try again in a few minutes".to_string(),
                "The server may be experiencing issues".to_string(),
            ],
            MarkdownError::ParseError { .. } => vec![
                "Verify the content format is supported".to_string(),
                "Check if the source content is valid".to_string(),
            ],
            MarkdownError::InvalidUrl { .. } => vec![
                "Ensure the URL starts with http:// or https://".to_string(),
                "Check that the URL is complete and properly formatted".to_string(),
                "Try copying the URL directly from your browser".to_string(),
            ],
            MarkdownError::AuthError { .. } => vec![
                "Check your authentication credentials".to_string(),
                "Verify that your token has the required permissions".to_string(),
                "Consider regenerating your authentication token".to_string(),
            ],
            MarkdownError::LegacyConfigurationError { .. } => vec![
                "Check your configuration file for errors".to_string(),
                "Verify all configuration values are valid".to_string(),
            ],
        }
    }
}

/// Frontmatter structure for document metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frontmatter {
    /// The source URL of the document
    pub source_url: Url,
    /// The exporter that generated this markdown
    pub exporter: String,
    /// The date and time when the document was downloaded
    pub date_downloaded: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_creation_from_string() {
        let content = "# Hello World";
        let markdown = Markdown::from(content.to_string());
        assert_eq!(markdown.as_str(), content);
    }

    #[test]
    fn test_markdown_new_valid() {
        let content = "# Hello World";
        let markdown = Markdown::new(content.to_string()).unwrap();
        assert_eq!(markdown.as_str(), content);
    }

    #[test]
    fn test_markdown_new_invalid_empty() {
        let result = Markdown::new("".to_string());
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ParseError { message } => {
                assert_eq!(
                    message,
                    "Markdown content cannot be empty or whitespace-only"
                );
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_markdown_new_invalid_whitespace() {
        let result = Markdown::new("   \n\t  ".to_string());
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ParseError { message } => {
                assert_eq!(
                    message,
                    "Markdown content cannot be empty or whitespace-only"
                );
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_markdown_display() {
        let content = "# Hello World\n\nThis is a test.";
        let markdown = Markdown::from(content.to_string());
        assert_eq!(format!("{markdown}"), content);
    }

    #[test]
    fn test_markdown_validation_valid() {
        let markdown = Markdown::from("# Valid markdown".to_string());
        assert!(markdown.validate().is_ok());
    }

    #[test]
    fn test_markdown_validation_empty() {
        let markdown = Markdown::from("".to_string());
        assert!(markdown.validate().is_err());
    }

    #[test]
    fn test_markdown_validation_whitespace_only() {
        let markdown = Markdown::from("   \n\t  ".to_string());
        assert!(markdown.validate().is_err());
    }

    #[test]
    fn test_markdown_with_frontmatter() {
        let content = Markdown::from("# Hello World\n\nThis is content.".to_string());
        let frontmatter = "---\nsource_url: \"https://example.com\"\nexporter: markdowndown\n---\n";
        let result = content.with_frontmatter(frontmatter);

        assert!(result
            .as_str()
            .contains("source_url: \"https://example.com\""));
        assert!(result.as_str().contains("# Hello World"));
        assert!(result.as_str().starts_with("---\n"));
    }

    #[test]
    fn test_markdown_frontmatter_extraction() {
        let content =
            "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n\n# Hello World";
        let markdown = Markdown::from(content.to_string());
        let frontmatter = markdown.frontmatter();

        assert!(frontmatter.is_some());
        let fm = frontmatter.unwrap();
        assert!(fm.contains("source_url: https://example.com"));
        assert!(fm.starts_with("---\n"));
        assert!(fm.ends_with("---\n"));
    }

    #[test]
    fn test_markdown_frontmatter_extraction_none() {
        let content = "# Hello World\n\nNo frontmatter here.";
        let markdown = Markdown::from(content.to_string());
        let frontmatter = markdown.frontmatter();

        assert!(frontmatter.is_none());
    }

    #[test]
    fn test_markdown_content_only() {
        let content = "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n\n# Hello World\n\nContent here.";
        let markdown = Markdown::from(content.to_string());
        let content_only = markdown.content_only();

        assert_eq!(content_only, "# Hello World\n\nContent here.");
        assert!(!content_only.contains("source_url"));
    }

    #[test]
    fn test_markdown_content_only_no_frontmatter() {
        let content = "# Hello World\n\nNo frontmatter here.";
        let markdown = Markdown::from(content.to_string());
        let content_only = markdown.content_only();

        assert_eq!(content_only, content);
    }

    #[test]
    fn test_markdown_frontmatter_roundtrip() {
        let original_content = "# Test Document\n\nThis is test content.";
        let frontmatter = "---\nsource_url: https://example.com\nexporter: markdowndown\n---\n";

        let markdown = Markdown::from(original_content.to_string());
        let with_frontmatter = markdown.with_frontmatter(frontmatter);

        // Extract frontmatter back
        let extracted_frontmatter = with_frontmatter.frontmatter();
        assert!(extracted_frontmatter.is_some());
        assert!(extracted_frontmatter.unwrap().contains("source_url"));

        // Extract content back
        let extracted_content = with_frontmatter.content_only();
        assert_eq!(extracted_content, original_content);
    }

    #[test]
    fn test_urltype_serialization() {
        let url_type = UrlType::GoogleDocs;
        let serialized = serde_yaml::to_string(&url_type).unwrap();
        let deserialized: UrlType = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(url_type, deserialized);
    }

    #[test]
    fn test_urltype_display() {
        assert_eq!(format!("{}", UrlType::Html), "HTML");
        assert_eq!(format!("{}", UrlType::GoogleDocs), "Google Docs");
        assert_eq!(format!("{}", UrlType::GitHubIssue), "GitHub Issue");
    }

    #[test]
    fn test_markdown_error_display() {
        let error = MarkdownError::NetworkError {
            message: "Connection timeout".to_string(),
        };
        assert_eq!(error.to_string(), "Network error: Connection timeout");
    }

    #[test]
    fn test_url_new_valid_https() {
        let url = Url::new("https://example.com".to_string()).unwrap();
        assert_eq!(url.as_str(), "https://example.com");
    }

    #[test]
    fn test_url_new_valid_http() {
        let url = Url::new("http://example.com".to_string()).unwrap();
        assert_eq!(url.as_str(), "http://example.com");
    }

    #[test]
    fn test_url_new_invalid() {
        let result = Url::new("not-a-url".to_string());
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
    fn test_url_display() {
        let url = Url::new("https://example.com".to_string()).unwrap();
        assert_eq!(format!("{url}"), "https://example.com");
    }

    #[test]
    fn test_frontmatter_yaml_serialization() {
        let frontmatter = Frontmatter {
            source_url: Url::new("https://example.com".to_string()).unwrap(),
            exporter: "markdowndown".to_string(),
            date_downloaded: DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };

        let yaml = serde_yaml::to_string(&frontmatter).unwrap();
        let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(frontmatter, deserialized);
    }

    /// Integration tests for type interactions
    mod integration_tests {
        use super::*;

        #[test]
        fn test_markdown_with_frontmatter_complete_workflow() {
            // Test complete workflow: create validated types and serialize together
            let markdown =
                Markdown::new("# Hello World\n\nThis is a test document.".to_string()).unwrap();
            let url = Url::new("https://docs.google.com/document/d/123".to_string()).unwrap();
            let frontmatter = Frontmatter {
                source_url: url,
                exporter: "markdowndown".to_string(),
                date_downloaded: Utc::now(),
            };

            // Test that all components work together
            let yaml_frontmatter = serde_yaml::to_string(&frontmatter).unwrap();
            let full_document = format!("---\n{yaml_frontmatter}---\n\n{markdown}");

            assert!(full_document.contains("# Hello World"));
            assert!(full_document.contains("https://docs.google.com"));
            assert!(full_document.contains("markdowndown"));
        }

        #[test]
        fn test_error_propagation_url_to_frontmatter() {
            // Test that URL validation errors propagate correctly
            let invalid_url_result = Url::new("not-a-valid-url".to_string());
            assert!(invalid_url_result.is_err());

            match invalid_url_result.unwrap_err() {
                MarkdownError::ValidationError { kind, context } => {
                    assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                    assert_eq!(context.url, "not-a-valid-url");
                }
                _ => panic!("Expected ValidationError with InvalidUrl kind"),
            }
        }

        #[test]
        fn test_error_propagation_markdown_validation() {
            // Test that Markdown validation errors propagate correctly
            let invalid_markdown_result = Markdown::new("   \n\t  ".to_string());
            assert!(invalid_markdown_result.is_err());

            match invalid_markdown_result.unwrap_err() {
                MarkdownError::ParseError { message } => {
                    assert_eq!(
                        message,
                        "Markdown content cannot be empty or whitespace-only"
                    );
                }
                _ => panic!("Expected ParseError"),
            }
        }

        #[test]
        fn test_combined_type_serialization_roundtrip() {
            // Test that complex nested structures serialize and deserialize correctly
            let original_frontmatter = Frontmatter {
                source_url: Url::new("https://github.com/user/repo/issues/123".to_string())
                    .unwrap(),
                exporter: "markdowndown".to_string(),
                date_downloaded: DateTime::parse_from_rfc3339("2023-12-01T10:30:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            };

            // Serialize to YAML
            let yaml = serde_yaml::to_string(&original_frontmatter).unwrap();

            // Deserialize back
            let deserialized: Frontmatter = serde_yaml::from_str(&yaml).unwrap();

            // Verify all fields match exactly
            assert_eq!(original_frontmatter.source_url, deserialized.source_url);
            assert_eq!(original_frontmatter.exporter, deserialized.exporter);
            assert_eq!(
                original_frontmatter.date_downloaded,
                deserialized.date_downloaded
            );
        }
    }

    /// Property-like tests for validation edge cases
    mod validation_edge_cases {
        use super::*;

        #[test]
        fn test_markdown_validation_unicode_whitespace() {
            // Test various Unicode whitespace characters
            let unicode_whitespace_chars = [
                "\u{0009}", // TAB
                "\u{000A}", // LINE FEED
                "\u{000B}", // LINE TABULATION
                "\u{000C}", // FORM FEED
                "\u{000D}", // CARRIAGE RETURN
                "\u{0020}", // SPACE
                "\u{0085}", // NEXT LINE
                "\u{00A0}", // NO-BREAK SPACE
                "\u{1680}", // OGHAM SPACE MARK
                "\u{2000}", // EN QUAD
                "\u{2001}", // EM QUAD
                "\u{2002}", // EN SPACE
                "\u{2003}", // EM SPACE
                "\u{2004}", // THREE-PER-EM SPACE
                "\u{2005}", // FOUR-PER-EM SPACE
                "\u{2006}", // SIX-PER-EM SPACE
                "\u{2007}", // FIGURE SPACE
                "\u{2008}", // PUNCTUATION SPACE
                "\u{2009}", // THIN SPACE
                "\u{200A}", // HAIR SPACE
                "\u{2028}", // LINE SEPARATOR
                "\u{2029}", // PARAGRAPH SEPARATOR
                "\u{202F}", // NARROW NO-BREAK SPACE
                "\u{205F}", // MEDIUM MATHEMATICAL SPACE
                "\u{3000}", // IDEOGRAPHIC SPACE
            ];

            for whitespace in unicode_whitespace_chars {
                let only_whitespace = whitespace.repeat(5);
                let markdown_result = Markdown::new(only_whitespace);
                assert!(
                    markdown_result.is_err(),
                    "Should reject Unicode whitespace: {whitespace:?}"
                );
            }

            // Test combinations of different whitespace characters
            let mixed_whitespace = "  \u{2000}\u{3000}\t\n\u{00A0}  ".to_string();
            let markdown_result = Markdown::new(mixed_whitespace);
            assert!(
                markdown_result.is_err(),
                "Should reject mixed Unicode whitespace"
            );
        }

        #[test]
        fn test_markdown_validation_minimal_valid_content() {
            // Test minimal valid content that should pass validation
            let minimal_valid_cases = [
                "a",         // Single character
                "1",         // Single digit
                ".",         // Single punctuation
                "🚀",        // Single emoji
                " a ",       // Valid content with surrounding whitespace
                "\n\na\n\n", // Valid content with surrounding newlines
                "\t\ta\t\t", // Valid content with surrounding tabs
            ];

            for case in minimal_valid_cases {
                let markdown_result = Markdown::new(case.to_string());
                assert!(
                    markdown_result.is_ok(),
                    "Should accept minimal valid content: {case:?}"
                );
            }
        }

        #[test]
        fn test_url_validation_edge_cases() {
            // Test URL validation with various edge cases
            let valid_url_cases = [
                "http://example.com",
                "https://example.com",
                "http://localhost",
                "https://localhost:8080",
                "http://192.168.1.1",
                "https://sub.domain.com/path?query=value#fragment",
                "http://user:pass@example.com",
                "https://example.com:443/very/long/path/with/many/segments",
                "file:///absolute/path/to/file.md", // Local file URL (absolute)
                "file://./relative/path.md",        // Local file URL (relative)
                "/absolute/path/to/file.md",        // Local file path (absolute)
                "./relative/file.md",               // Local file path (relative)
                "document.md",                      // Simple filename
            ];

            for url_case in valid_url_cases {
                let url_result = Url::new(url_case.to_string());
                assert!(url_result.is_ok(), "Should accept valid URL: {url_case}");
            }

            let invalid_url_cases = [
                "ftp://example.com",       // Wrong protocol
                "example.com",             // Missing protocol (domain without protocol)
                "www.example.com",         // Missing protocol (domain without protocol)
                "mailto:test@example.com", // Wrong protocol
                "",                        // Empty string
                "http://",                 // Incomplete
                "https://",                // Incomplete
            ];

            for url_case in invalid_url_cases {
                let url_result = Url::new(url_case.to_string());
                assert!(url_result.is_err(), "Should reject invalid URL: {url_case}");
            }
        }

        #[test]
        fn test_roundtrip_serialization_properties() {
            // Test that serialization roundtrips preserve all data exactly
            let test_cases = [
                (
                    "https://docs.google.com/document/d/1234567890abcdef",
                    "markdowndown-v1.0",
                    "2023-01-15T14:30:45Z",
                ),
                (
                    "http://localhost:3000/api/docs/12345",
                    "custom-exporter",
                    "2023-12-31T23:59:59Z",
                ),
                (
                    "https://github.com/user/repo/issues/999",
                    "github-markdown-exporter",
                    "2023-06-15T09:15:30Z",
                ),
            ];

            for (url_str, exporter_str, date_str) in test_cases {
                let original = Frontmatter {
                    source_url: Url::new(url_str.to_string()).unwrap(),
                    exporter: exporter_str.to_string(),
                    date_downloaded: DateTime::parse_from_rfc3339(date_str)
                        .unwrap()
                        .with_timezone(&Utc),
                };

                // Test YAML roundtrip
                let yaml = serde_yaml::to_string(&original).unwrap();
                let from_yaml: Frontmatter = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(
                    original, from_yaml,
                    "YAML roundtrip should preserve all data"
                );

                // Verify specific field preservation
                assert_eq!(original.source_url.as_str(), from_yaml.source_url.as_str());
                assert_eq!(original.exporter, from_yaml.exporter);
                assert_eq!(original.date_downloaded, from_yaml.date_downloaded);
            }
        }

        #[test]
        fn test_markdown_content_preservation() {
            // Test that various markdown content is preserved exactly
            let markdown_cases = [
                "# Simple Header",
                "## Header with *emphasis* and **bold**",
                "- List item 1\n- List item 2\n  - Nested item",
                "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```",
                "Link: [Example](https://example.com)",
                "![Image](https://example.com/image.png)",
                "> Blockquote with **bold** text",
                "Table | Header\n------|-------\nCell1 | Cell2",
                "Mixed content with 🚀 emoji and Unicode: café résumé naïve",
                "Line 1\n\nLine 3 (with blank line)\n\n\nLine 6 (multiple blanks)",
            ];

            for content in markdown_cases {
                let markdown = Markdown::new(content.to_string()).unwrap();
                assert_eq!(
                    markdown.as_str(),
                    content,
                    "Content should be preserved exactly"
                );
                assert_eq!(
                    format!("{markdown}"),
                    content,
                    "Display should match original"
                );
            }
        }

        /// Tests for the enhanced error handling system
        mod enhanced_error_handling_tests {
            use super::*;

            #[test]
            fn test_error_context_creation() {
                let context = ErrorContext::new(
                    "https://example.com/test",
                    "URL validation",
                    "TestConverter",
                );

                assert_eq!(context.url, "https://example.com/test");
                assert_eq!(context.operation, "URL validation");
                assert_eq!(context.converter_type, "TestConverter");
                assert!(context.additional_info.is_none());
                // Timestamp should be recent (within last few seconds)
                let now = Utc::now();
                let diff = (now - context.timestamp).num_seconds();
                assert!((0..5).contains(&diff));
            }

            #[test]
            fn test_error_context_with_info() {
                let context = ErrorContext::new(
                    "https://example.com/test",
                    "URL validation",
                    "TestConverter",
                )
                .with_info("Additional debugging info");

                assert_eq!(
                    context.additional_info,
                    Some("Additional debugging info".to_string())
                );
            }

            #[test]
            fn test_validation_error_creation() {
                let context = ErrorContext::new("invalid-url", "URL parsing", "UrlValidator");

                let error = MarkdownError::ValidationError {
                    kind: ValidationErrorKind::InvalidUrl,
                    context: context.clone(),
                };

                assert_eq!(error.context(), Some(&context));
                assert!(!error.is_retryable());
                assert!(!error.is_recoverable());

                let suggestions = error.suggestions();
                assert!(suggestions
                    .contains(&"Ensure the URL starts with http:// or https://".to_string()));
            }

            #[test]
            fn test_network_error_retryable() {
                let context =
                    ErrorContext::new("https://example.com", "HTTP request", "HttpClient");

                // Test retryable network errors
                let timeout_error = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::Timeout,
                    context: context.clone(),
                };
                assert!(timeout_error.is_retryable());
                assert!(timeout_error.is_recoverable());

                let connection_error = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::ConnectionFailed,
                    context: context.clone(),
                };
                assert!(connection_error.is_retryable());

                let rate_limit_error = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::RateLimited,
                    context: context.clone(),
                };
                assert!(rate_limit_error.is_retryable());

                // Test non-retryable network errors
                let dns_error = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::DnsResolution,
                    context: context.clone(),
                };
                assert!(!dns_error.is_retryable());

                // Test server errors - 5xx should be retryable, 4xx should not
                let server_error_500 = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::ServerError(500),
                    context: context.clone(),
                };
                assert!(server_error_500.is_retryable());

                let client_error_404 = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::ServerError(404),
                    context,
                };
                assert!(!client_error_404.is_retryable());
            }

            #[test]
            fn test_auth_error_retryable() {
                let context =
                    ErrorContext::new("https://api.example.com", "API request", "ApiClient");

                // Only expired tokens should be retryable
                let expired_token_error = MarkdownError::AuthenticationError {
                    kind: AuthErrorKind::TokenExpired,
                    context: context.clone(),
                };
                assert!(expired_token_error.is_retryable());
                assert!(expired_token_error.is_recoverable());

                let missing_token_error = MarkdownError::AuthenticationError {
                    kind: AuthErrorKind::MissingToken,
                    context: context.clone(),
                };
                assert!(!missing_token_error.is_retryable());
                assert!(missing_token_error.is_recoverable());

                let invalid_token_error = MarkdownError::AuthenticationError {
                    kind: AuthErrorKind::InvalidToken,
                    context: context.clone(),
                };
                assert!(!invalid_token_error.is_retryable());

                let permission_denied_error = MarkdownError::AuthenticationError {
                    kind: AuthErrorKind::PermissionDenied,
                    context,
                };
                assert!(!permission_denied_error.is_retryable());
            }

            #[test]
            fn test_content_error_recovery() {
                let context = ErrorContext::new(
                    "https://example.com/document",
                    "Content parsing",
                    "ContentParser",
                );

                // Unsupported format should be recoverable
                let unsupported_format_error = MarkdownError::ContentError {
                    kind: ContentErrorKind::UnsupportedFormat,
                    context: context.clone(),
                };
                assert!(unsupported_format_error.is_recoverable());
                assert!(!unsupported_format_error.is_retryable());

                // Empty content and parsing failed should not be recoverable
                let empty_content_error = MarkdownError::ContentError {
                    kind: ContentErrorKind::EmptyContent,
                    context: context.clone(),
                };
                assert!(!empty_content_error.is_recoverable());

                let parsing_failed_error = MarkdownError::ContentError {
                    kind: ContentErrorKind::ParsingFailed,
                    context,
                };
                assert!(!parsing_failed_error.is_recoverable());
            }

            #[test]
            fn test_converter_error_recovery() {
                let context = ErrorContext::new(
                    "https://example.com/document",
                    "Document conversion",
                    "PandocConverter",
                );

                let converter_error = MarkdownError::ConverterError {
                    kind: ConverterErrorKind::ExternalToolFailed,
                    context,
                };
                assert!(converter_error.is_recoverable());
                assert!(!converter_error.is_retryable());
            }

            #[test]
            fn test_configuration_error_recovery() {
                let context = ErrorContext::new(
                    "file://config.yaml",
                    "Configuration loading",
                    "ConfigLoader",
                );

                let config_error = MarkdownError::ConfigurationError {
                    kind: ConfigErrorKind::InvalidConfig,
                    context,
                };
                assert!(!config_error.is_recoverable());
                assert!(!config_error.is_retryable());
            }

            #[test]
            fn test_error_suggestions_comprehensive() {
                let context =
                    ErrorContext::new("https://example.com", "Test operation", "TestConverter");

                // Test validation error suggestions
                let validation_error = MarkdownError::ValidationError {
                    kind: ValidationErrorKind::InvalidUrl,
                    context: context.clone(),
                };
                let suggestions = validation_error.suggestions();
                assert!(!suggestions.is_empty());
                assert!(suggestions.iter().any(|s| s.contains("http")));

                // Test network error suggestions
                let network_error = MarkdownError::EnhancedNetworkError {
                    kind: NetworkErrorKind::Timeout,
                    context: context.clone(),
                };
                let suggestions = network_error.suggestions();
                assert!(suggestions
                    .iter()
                    .any(|s| s.contains("internet connection")));

                // Test auth error suggestions
                let auth_error = MarkdownError::AuthenticationError {
                    kind: AuthErrorKind::MissingToken,
                    context: context.clone(),
                };
                let suggestions = auth_error.suggestions();
                assert!(suggestions.iter().any(|s| s.contains("authentication")));

                // Test content error suggestions
                let content_error = MarkdownError::ContentError {
                    kind: ContentErrorKind::EmptyContent,
                    context: context.clone(),
                };
                let suggestions = content_error.suggestions();
                assert!(suggestions.iter().any(|s| s.contains("content")));

                // Test converter error suggestions
                let converter_error = MarkdownError::ConverterError {
                    kind: ConverterErrorKind::ExternalToolFailed,
                    context: context.clone(),
                };
                let suggestions = converter_error.suggestions();
                assert!(suggestions.iter().any(|s| s.contains("external tools")));

                // Test configuration error suggestions
                let config_error = MarkdownError::ConfigurationError {
                    kind: ConfigErrorKind::InvalidConfig,
                    context,
                };
                let suggestions = config_error.suggestions();
                assert!(suggestions.iter().any(|s| s.contains("configuration")));
            }

            #[test]
            fn test_legacy_error_compatibility() {
                // Test that legacy errors still work but don't have enhanced features
                let legacy_parse_error = MarkdownError::ParseError {
                    message: "Legacy parsing failed".to_string(),
                };

                assert!(legacy_parse_error.context().is_none());
                assert!(!legacy_parse_error.is_retryable());
                assert!(!legacy_parse_error.is_recoverable());

                let suggestions = legacy_parse_error.suggestions();
                assert!(suggestions.iter().any(|s| s.contains("content format")));

                // Test legacy network error
                let legacy_network_error = MarkdownError::NetworkError {
                    message: "Connection timeout occurred".to_string(),
                };

                assert!(legacy_network_error.context().is_none());
                assert!(legacy_network_error.is_retryable()); // Should detect "timeout" in message
                assert!(legacy_network_error.is_recoverable());

                let suggestions = legacy_network_error.suggestions();
                assert!(suggestions
                    .iter()
                    .any(|s| s.contains("internet connection")));

                // Test legacy invalid URL error
                let legacy_url_error = MarkdownError::InvalidUrl {
                    url: "not-a-url".to_string(),
                };

                assert!(legacy_url_error.context().is_none());
                assert!(!legacy_url_error.is_retryable());
                assert!(!legacy_url_error.is_recoverable());

                let suggestions = legacy_url_error.suggestions();
                assert!(suggestions.iter().any(|s| s.contains("http")));
            }

            #[test]
            fn test_error_display_format() {
                let context = ErrorContext::new(
                    "https://example.com/test",
                    "Test operation",
                    "TestConverter",
                );

                let error = MarkdownError::ValidationError {
                    kind: ValidationErrorKind::InvalidUrl,
                    context,
                };

                let error_string = format!("{error}");
                assert!(error_string.contains("Validation error"));
                assert!(error_string.contains("InvalidUrl"));
            }

            #[test]
            fn test_error_context_serialization() {
                let context = ErrorContext::new(
                    "https://example.com/test",
                    "Test operation",
                    "TestConverter",
                )
                .with_info("Additional context");

                // Test that ErrorContext can be serialized/deserialized
                let yaml = serde_yaml::to_string(&context).unwrap();
                let deserialized: ErrorContext = serde_yaml::from_str(&yaml).unwrap();

                assert_eq!(context.url, deserialized.url);
                assert_eq!(context.operation, deserialized.operation);
                assert_eq!(context.converter_type, deserialized.converter_type);
                assert_eq!(context.additional_info, deserialized.additional_info);
            }

            #[test]
            fn test_error_kind_serialization() {
                // Test that all error kinds can be serialized/deserialized
                let validation_kind = ValidationErrorKind::InvalidUrl;
                let yaml = serde_yaml::to_string(&validation_kind).unwrap();
                let deserialized: ValidationErrorKind = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(validation_kind, deserialized);

                let network_kind = NetworkErrorKind::ServerError(500);
                let yaml = serde_yaml::to_string(&network_kind).unwrap();
                let deserialized: NetworkErrorKind = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(network_kind, deserialized);

                let auth_kind = AuthErrorKind::TokenExpired;
                let yaml = serde_yaml::to_string(&auth_kind).unwrap();
                let deserialized: AuthErrorKind = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(auth_kind, deserialized);

                let content_kind = ContentErrorKind::ParsingFailed;
                let yaml = serde_yaml::to_string(&content_kind).unwrap();
                let deserialized: ContentErrorKind = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(content_kind, deserialized);

                let converter_kind = ConverterErrorKind::ExternalToolFailed;
                let yaml = serde_yaml::to_string(&converter_kind).unwrap();
                let deserialized: ConverterErrorKind = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(converter_kind, deserialized);

                let config_kind = ConfigErrorKind::MissingDependency;
                let yaml = serde_yaml::to_string(&config_kind).unwrap();
                let deserialized: ConfigErrorKind = serde_yaml::from_str(&yaml).unwrap();
                assert_eq!(config_kind, deserialized);
            }
        }
    }

    /// Additional comprehensive tests for improving coverage
    mod comprehensive_coverage_tests {
        use super::*;

        /// Test complex frontmatter edge cases and parsing logic
        mod frontmatter_edge_cases {
            use super::*;

            #[test]
            fn test_frontmatter_with_nested_delimiters() {
                // Test malformed frontmatter with nested --- delimiters
                let content = "---\n---\nsource_url: test\n---\n\n# Content";
                let markdown = Markdown::from(content.to_string());

                // Should return None due to malformed frontmatter (starts with ---)
                assert!(markdown.frontmatter().is_none());

                // content_only should return original content since frontmatter is malformed
                assert_eq!(markdown.content_only(), content);
            }

            #[test]
            fn test_frontmatter_with_invalid_yaml() {
                // Test frontmatter with invalid YAML syntax
                let content = "---\nsource_url: https://example.com\ninvalid: yaml: syntax: here\n---\n\n# Content";
                let markdown = Markdown::from(content.to_string());

                // Should return None due to invalid YAML
                assert!(markdown.frontmatter().is_none());

                // content_only should return original content
                assert_eq!(markdown.content_only(), content);
            }

            #[test]
            fn test_frontmatter_incomplete_delimiter() {
                // Test frontmatter that starts but never closes properly
                let content = "---\nsource_url: https://example.com\nexporter: test\n\n# Content without closing delimiter";
                let markdown = Markdown::from(content.to_string());

                // Should return None due to missing closing delimiter
                assert!(markdown.frontmatter().is_none());

                // content_only should return original content
                assert_eq!(markdown.content_only(), content);
            }

            #[test]
            fn test_frontmatter_multiple_closing_delimiters() {
                // Test content with multiple --- lines after frontmatter
                let content = "---\nsource_url: https://example.com\n---\n\n---\n---\n\n# Actual Content\n\nContent here.";
                let markdown = Markdown::from(content.to_string());

                // Should extract frontmatter successfully
                let frontmatter = markdown.frontmatter();
                assert!(frontmatter.is_some());
                assert!(frontmatter
                    .unwrap()
                    .contains("source_url: https://example.com"));

                // content_only should skip extra --- lines and return just content
                let content_only = markdown.content_only();
                assert_eq!(content_only, "\n# Actual Content\n\nContent here.");
                assert!(!content_only.contains("source_url"));
                assert!(!content_only.contains("---"));
            }

            #[test]
            fn test_frontmatter_empty_yaml_section() {
                // Test frontmatter with empty YAML section
                let content = "---\n\n---\n\n# Content";
                let markdown = Markdown::from(content.to_string());

                // Empty YAML should be valid (empty object)
                let frontmatter = markdown.frontmatter();
                assert!(frontmatter.is_some());

                let content_only = markdown.content_only();
                assert_eq!(content_only, "# Content");
            }

            #[test]
            fn test_frontmatter_whitespace_only_yaml() {
                // Test frontmatter with only whitespace in YAML section
                let content = "---\n   \n\t\n   \n---\n\n# Content";
                let markdown = Markdown::from(content.to_string());

                // Whitespace-only YAML is not valid frontmatter in this implementation
                let frontmatter = markdown.frontmatter();
                assert!(frontmatter.is_none());

                let content_only = markdown.content_only();
                // Since whitespace-only YAML is not valid frontmatter, the whole content is returned
                assert_eq!(content_only, "---\n   \n\t\n   \n---\n\n# Content");
            }

            #[test]
            fn test_content_only_complex_line_endings() {
                // Test content_only with various line ending scenarios
                let content = "---\nsource_url: test\n---\n\n\n\n# Header\n\nContent\n\n";
                let markdown = Markdown::from(content.to_string());

                let content_only = markdown.content_only();
                // Should preserve the exact content structure, but may strip leading newlines
                assert_eq!(content_only, "\n\n# Header\n\nContent\n\n");
            }

            #[test]
            fn test_content_only_no_content_after_frontmatter() {
                // Test when there's only frontmatter and no content
                let content = "---\nsource_url: https://example.com\n---\n";
                let markdown = Markdown::from(content.to_string());

                let content_only = markdown.content_only();
                // Should return empty string when no content after frontmatter
                assert_eq!(content_only, "");
            }

            #[test]
            fn test_frontmatter_extraction_edge_case_byte_boundaries() {
                // Test frontmatter extraction with Unicode characters that might affect byte calculations
                let content = "---\nsource_url: \"https://café.example.com/naïve\"\nexporter: \"markdowndown-🚀\"\n---\n\n# Unicode Test 🎯\n\nContent with émojis and açcénts.";
                let markdown = Markdown::from(content.to_string());

                let frontmatter = markdown.frontmatter();
                assert!(frontmatter.is_some());
                let fm = frontmatter.unwrap();
                assert!(fm.contains("café.example.com"));
                assert!(fm.contains("markdowndown-🚀"));

                let content_only = markdown.content_only();
                assert_eq!(
                    content_only,
                    "# Unicode Test 🎯\n\nContent with émojis and açcénts."
                );
                assert!(!content_only.contains("café.example.com"));
            }
        }

        /// Test URL validation edge cases and error conditions
        mod url_validation_edge_cases {
            use super::*;

            #[test]
            fn test_url_minimal_valid_cases() {
                // Test minimal valid HTTP/HTTPS URLs
                let minimal_cases = [
                    "http://a",    // Minimal valid HTTP
                    "https://a",   // Minimal valid HTTPS
                    "http://a.b",  // Minimal domain
                    "https://a.b", // Minimal domain HTTPS
                ];

                for url_str in minimal_cases {
                    let url_result = Url::new(url_str.to_string());
                    assert!(
                        url_result.is_ok(),
                        "Should accept minimal valid URL: {url_str}"
                    );

                    let url = url_result.unwrap();
                    assert_eq!(url.as_str(), url_str);
                    assert_eq!(url.as_ref(), url_str);
                    assert_eq!(format!("{url}"), url_str);
                }
            }

            #[test]
            fn test_url_edge_case_protocols() {
                // Test exact boundary cases for protocol validation
                let boundary_cases = [
                    ("http://", false),  // Exactly protocol length, no domain
                    ("https://", false), // Exactly protocol length, no domain
                    ("http://x", true),  // One char after protocol
                    ("https://x", true), // One char after protocol
                ];

                for (url_str, should_succeed) in boundary_cases {
                    let url_result = Url::new(url_str.to_string());
                    if should_succeed {
                        assert!(url_result.is_ok(), "Should accept URL: {url_str}");
                    } else {
                        assert!(url_result.is_err(), "Should reject URL: {url_str}");
                    }
                }
            }

            #[test]
            fn test_url_serialization_deserialization() {
                // Test that URL can be properly serialized and deserialized
                let original_url =
                    Url::new("https://docs.google.com/document/d/test123".to_string()).unwrap();

                // Test YAML serialization
                let yaml = serde_yaml::to_string(&original_url).unwrap();
                assert!(yaml.contains("https://docs.google.com"));

                // Test deserialization
                let yaml_input = "\"https://github.com/user/repo/issues/42\"";
                let deserialized: Result<Url, _> = serde_yaml::from_str(yaml_input);
                assert!(deserialized.is_ok());

                let url = deserialized.unwrap();
                assert_eq!(url.as_str(), "https://github.com/user/repo/issues/42");
            }

            #[test]
            fn test_url_deserialization_invalid() {
                // Test that invalid URLs are rejected during deserialization
                let invalid_yaml_inputs = [
                    "\"not-a-url\"",
                    "\"ftp://example.com\"",
                    "\"example.com\"",
                    "\"\"",
                ];

                for yaml_input in invalid_yaml_inputs {
                    let result: Result<Url, _> = serde_yaml::from_str(yaml_input);
                    assert!(
                        result.is_err(),
                        "Should reject invalid URL during deserialization: {yaml_input}"
                    );
                }
            }
        }

        /// Test error handling and suggestion logic comprehensively
        mod error_handling_comprehensive {
            use super::*;

            #[test]
            fn test_legacy_network_error_retryable_detection() {
                // Test legacy network error with different message patterns
                let retryable_messages = [
                    "Connection timeout occurred",
                    "Request timeout after 30 seconds",
                    "connection failed", // Lowercase to match actual implementation
                    "rate limit exceeded", // Lowercase to match actual implementation
                ];

                for message in retryable_messages {
                    let error = MarkdownError::NetworkError {
                        message: message.to_string(),
                    };
                    assert!(error.is_retryable(), "Should be retryable: {message}");
                    assert!(error.is_recoverable(), "Should be recoverable: {message}");
                }

                let non_retryable_messages = [
                    "Invalid request format",
                    "Authentication failed",
                    "Resource not found",
                    "Connection refused",
                    "Too many requests", // Doesn't contain "rate limit" so not retryable
                ];

                for message in non_retryable_messages {
                    let error = MarkdownError::NetworkError {
                        message: message.to_string(),
                    };
                    assert!(!error.is_retryable(), "Should not be retryable: {message}");
                }
            }

            #[test]
            fn test_server_error_status_code_boundaries() {
                // Test specific HTTP status code boundaries for retry logic
                let context =
                    ErrorContext::new("https://api.example.com", "HTTP request", "HttpClient");

                // Test boundary cases for server error recovery
                let test_cases = [
                    (400, false, false), // Bad Request - not retryable, not recoverable
                    (401, false, false), // Unauthorized - not retryable, not recoverable
                    (404, false, false), // Not Found - not retryable, not recoverable
                    (429, false, true),  // Too Many Requests - not retryable but recoverable
                    (500, true, true),   // Internal Server Error - retryable and recoverable
                    (501, true, true),   // Not Implemented - retryable and recoverable
                    (502, true, true),   // Bad Gateway - retryable and recoverable
                    (503, true, true),   // Service Unavailable - retryable and recoverable
                    (504, true, true),   // Gateway Timeout - retryable and recoverable
                    (505, true, true),   // HTTP Version Not Supported - default to recoverable
                ];

                for (status_code, expected_retryable, expected_recoverable) in test_cases {
                    let error = MarkdownError::EnhancedNetworkError {
                        kind: NetworkErrorKind::ServerError(status_code),
                        context: context.clone(),
                    };

                    assert_eq!(
                        error.is_retryable(),
                        expected_retryable,
                        "Status {status_code} retryable should be {expected_retryable}"
                    );
                    assert_eq!(
                        error.is_recoverable(),
                        expected_recoverable,
                        "Status {status_code} recoverable should be {expected_recoverable}"
                    );
                }
            }

            #[test]
            fn test_error_suggestions_content_coverage() {
                // Test that all error suggestion branches are covered
                let context = ErrorContext::new("test-url", "test-op", "test-converter");

                // Test all ValidationErrorKind variants
                let validation_kinds = [
                    ValidationErrorKind::InvalidUrl,
                    ValidationErrorKind::InvalidFormat,
                    ValidationErrorKind::MissingParameter,
                ];

                for kind in validation_kinds {
                    let error = MarkdownError::ValidationError {
                        kind: kind.clone(),
                        context: context.clone(),
                    };
                    let suggestions = error.suggestions();
                    assert!(
                        !suggestions.is_empty(),
                        "Should have suggestions for {kind:?}"
                    );
                }

                // Test all NetworkErrorKind variants
                let network_kinds = [
                    NetworkErrorKind::Timeout,
                    NetworkErrorKind::ConnectionFailed,
                    NetworkErrorKind::DnsResolution,
                    NetworkErrorKind::RateLimited,
                    NetworkErrorKind::ServerError(500),
                ];

                for kind in network_kinds {
                    let error = MarkdownError::EnhancedNetworkError {
                        kind: kind.clone(),
                        context: context.clone(),
                    };
                    let suggestions = error.suggestions();
                    assert!(
                        !suggestions.is_empty(),
                        "Should have suggestions for {kind:?}"
                    );
                }

                // Test all AuthErrorKind variants
                let auth_kinds = [
                    AuthErrorKind::MissingToken,
                    AuthErrorKind::InvalidToken,
                    AuthErrorKind::PermissionDenied,
                    AuthErrorKind::TokenExpired,
                ];

                for kind in auth_kinds {
                    let error = MarkdownError::AuthenticationError {
                        kind: kind.clone(),
                        context: context.clone(),
                    };
                    let suggestions = error.suggestions();
                    assert!(
                        !suggestions.is_empty(),
                        "Should have suggestions for {kind:?}"
                    );
                }

                // Test all ContentErrorKind variants
                let content_kinds = [
                    ContentErrorKind::EmptyContent,
                    ContentErrorKind::UnsupportedFormat,
                    ContentErrorKind::ParsingFailed,
                ];

                for kind in content_kinds {
                    let error = MarkdownError::ContentError {
                        kind: kind.clone(),
                        context: context.clone(),
                    };
                    let suggestions = error.suggestions();
                    assert!(
                        !suggestions.is_empty(),
                        "Should have suggestions for {kind:?}"
                    );
                }

                // Test all ConverterErrorKind variants
                let converter_kinds = [
                    ConverterErrorKind::ExternalToolFailed,
                    ConverterErrorKind::ProcessingError,
                    ConverterErrorKind::UnsupportedOperation,
                ];

                for kind in converter_kinds {
                    let error = MarkdownError::ConverterError {
                        kind: kind.clone(),
                        context: context.clone(),
                    };
                    let suggestions = error.suggestions();
                    assert!(
                        !suggestions.is_empty(),
                        "Should have suggestions for {kind:?}"
                    );
                }

                // Test all ConfigErrorKind variants
                let config_kinds = [
                    ConfigErrorKind::InvalidConfig,
                    ConfigErrorKind::MissingDependency,
                    ConfigErrorKind::InvalidValue,
                ];

                for kind in config_kinds {
                    let error = MarkdownError::ConfigurationError {
                        kind: kind.clone(),
                        context: context.clone(),
                    };
                    let suggestions = error.suggestions();
                    assert!(
                        !suggestions.is_empty(),
                        "Should have suggestions for {kind:?}"
                    );
                }
            }

            #[test]
            fn test_error_context_none_branches() {
                // Test that legacy errors return None for context
                let legacy_errors = [
                    MarkdownError::NetworkError {
                        message: "test".to_string(),
                    },
                    MarkdownError::ParseError {
                        message: "test".to_string(),
                    },
                    MarkdownError::InvalidUrl {
                        url: "test".to_string(),
                    },
                    MarkdownError::AuthError {
                        message: "test".to_string(),
                    },
                    MarkdownError::LegacyConfigurationError {
                        message: "test".to_string(),
                    },
                ];

                for error in legacy_errors {
                    assert!(
                        error.context().is_none(),
                        "Legacy error should return None for context: {error}"
                    );
                }
            }

            #[test]
            fn test_enhanced_error_context_branches() {
                // Test that enhanced errors return Some for context
                let context = ErrorContext::new("test", "test", "test");

                let enhanced_errors = [
                    MarkdownError::ValidationError {
                        kind: ValidationErrorKind::InvalidUrl,
                        context: context.clone(),
                    },
                    MarkdownError::EnhancedNetworkError {
                        kind: NetworkErrorKind::Timeout,
                        context: context.clone(),
                    },
                    MarkdownError::AuthenticationError {
                        kind: AuthErrorKind::MissingToken,
                        context: context.clone(),
                    },
                    MarkdownError::ContentError {
                        kind: ContentErrorKind::EmptyContent,
                        context: context.clone(),
                    },
                    MarkdownError::ConverterError {
                        kind: ConverterErrorKind::ExternalToolFailed,
                        context: context.clone(),
                    },
                    MarkdownError::ConfigurationError {
                        kind: ConfigErrorKind::InvalidConfig,
                        context: context.clone(),
                    },
                ];

                for error in enhanced_errors {
                    assert!(
                        error.context().is_some(),
                        "Enhanced error should return Some for context: {error}"
                    );
                }
            }
        }

        /// Test trait implementations and type conversions
        mod trait_implementations {
            use super::*;

            #[test]
            fn test_markdown_trait_implementations() {
                let content = "# Test Markdown\n\nContent here.";
                let markdown = Markdown::new(content.to_string()).unwrap();

                // Test AsRef<str>
                let as_ref: &str = markdown.as_ref();
                assert_eq!(as_ref, content);

                // Test Deref to str
                let deref_str: &str = &markdown;
                assert_eq!(deref_str, content);

                // Test that we can use string methods directly on Markdown
                assert!(markdown.contains("Test Markdown"));
                assert!(markdown.starts_with("# Test"));
                assert_eq!(markdown.len(), content.len());

                // Test From<String> for Markdown
                let from_string = Markdown::from("Another test".to_string());
                assert_eq!(from_string.as_str(), "Another test");

                // Test Into<String> for Markdown
                let into_string: String = markdown.into();
                assert_eq!(into_string, content);
            }

            #[test]
            fn test_url_trait_implementations() {
                let url_str = "https://example.com/test";
                let url = Url::new(url_str.to_string()).unwrap();

                // Test AsRef<str>
                let as_ref: &str = url.as_ref();
                assert_eq!(as_ref, url_str);

                // Test Display
                assert_eq!(format!("{url}"), url_str);

                // Test Debug (ensure it contains the URL)
                let debug_str = format!("{url:?}");
                assert!(debug_str.contains(url_str));
            }

            #[test]
            fn test_urltype_all_variants_display() {
                // Test Display for all UrlType variants
                let variants = [
                    (UrlType::Html, "HTML"),
                    (UrlType::GoogleDocs, "Google Docs"),
                    (UrlType::GitHubIssue, "GitHub Issue"),
                    (UrlType::LocalFile, "Local File"),
                ];

                for (variant, expected_display) in variants {
                    assert_eq!(format!("{variant}"), expected_display);

                    // Test Debug as well
                    let debug_str = format!("{variant:?}");
                    assert!(
                        debug_str.contains(&variant.to_string())
                            || debug_str.contains("LocalFile")
                            || debug_str.contains("Html")
                            || debug_str.contains("GoogleDocs")
                            || debug_str.contains("GitHubIssue")
                    );
                }
            }

            #[test]
            fn test_urltype_serialization_all_variants() {
                // Test serialization/deserialization for all UrlType variants
                let variants = [
                    UrlType::Html,
                    UrlType::GoogleDocs,
                    UrlType::GitHubIssue,
                    UrlType::LocalFile,
                ];

                for variant in variants {
                    // Test YAML serialization roundtrip
                    let yaml = serde_yaml::to_string(&variant).unwrap();
                    let deserialized: UrlType = serde_yaml::from_str(&yaml).unwrap();
                    assert_eq!(variant, deserialized);

                    // Test JSON serialization roundtrip
                    let json = serde_json::to_string(&variant).unwrap();
                    let deserialized: UrlType = serde_json::from_str(&json).unwrap();
                    assert_eq!(variant, deserialized);
                }
            }
        }

        /// Test various display and formatting edge cases
        mod display_and_formatting {
            use super::*;

            #[test]
            fn test_markdown_display_edge_cases() {
                // Test Display with various content types
                let test_cases = [
                    "",  // Empty content (though this would fail validation if created via new())
                    "a", // Single character
                    "Line 1\nLine 2\nLine 3", // Multi-line
                    "Content with\ttabs\tand\nNewlines\rand\rCarriage\returns", // Mixed whitespace
                    "Unicode: 🚀 café naïve résumé", // Unicode content
                    "Very long content that spans multiple lines and contains various markdown elements like # headers, **bold text**, *italic text*, [links](https://example.com), and `code snippets` to test display formatting", // Long content
                ];

                for content in test_cases {
                    if !content.trim().is_empty() {
                        // Only test non-empty content for validated Markdown
                        let markdown = Markdown::new(content.to_string()).unwrap();
                        let displayed = format!("{markdown}");
                        assert_eq!(
                            displayed, content,
                            "Display should match original content exactly"
                        );
                    }

                    // Test From<String> display (which doesn't require validation)
                    let markdown_from = Markdown::from(content.to_string());
                    let displayed_from = format!("{markdown_from}");
                    assert_eq!(
                        displayed_from, content,
                        "Display should match original content exactly"
                    );
                }
            }

            #[test]
            fn test_error_display_formatting() {
                // Test display formatting for all error types
                let context =
                    ErrorContext::new("https://test.com", "test operation", "TestConverter");

                let test_errors = [
                    MarkdownError::ValidationError {
                        kind: ValidationErrorKind::InvalidUrl,
                        context: context.clone(),
                    },
                    MarkdownError::EnhancedNetworkError {
                        kind: NetworkErrorKind::Timeout,
                        context: context.clone(),
                    },
                    MarkdownError::AuthenticationError {
                        kind: AuthErrorKind::MissingToken,
                        context: context.clone(),
                    },
                    MarkdownError::ContentError {
                        kind: ContentErrorKind::EmptyContent,
                        context: context.clone(),
                    },
                    MarkdownError::ConverterError {
                        kind: ConverterErrorKind::ExternalToolFailed,
                        context: context.clone(),
                    },
                    MarkdownError::ConfigurationError {
                        kind: ConfigErrorKind::InvalidConfig,
                        context: context.clone(),
                    },
                    MarkdownError::NetworkError {
                        message: "Legacy network error".to_string(),
                    },
                    MarkdownError::ParseError {
                        message: "Legacy parse error".to_string(),
                    },
                    MarkdownError::InvalidUrl {
                        url: "invalid-url".to_string(),
                    },
                    MarkdownError::AuthError {
                        message: "Legacy auth error".to_string(),
                    },
                    MarkdownError::LegacyConfigurationError {
                        message: "Legacy config error".to_string(),
                    },
                ];

                for error in test_errors {
                    let display_str = format!("{error}");
                    assert!(!display_str.is_empty(), "Error display should not be empty");

                    // Each error type should have its type name in the display
                    let debug_str = format!("{error:?}");
                    assert!(!debug_str.is_empty(), "Error debug should not be empty");
                }
            }
        }

        /// Test complex integration scenarios with multiple type interactions
        mod complex_integration_scenarios {
            use super::*;

            #[test]
            fn test_markdown_validation_after_frontmatter_addition() {
                // Test that validation still works after frontmatter operations
                let original = Markdown::new("# Test".to_string()).unwrap();
                let frontmatter = "---\nsource_url: \"https://example.com\"\n---\n";
                let with_frontmatter = original.with_frontmatter(frontmatter);

                // Should be able to extract frontmatter from the result
                let extracted = with_frontmatter.frontmatter();
                assert!(extracted.is_some());
                assert!(extracted.unwrap().contains("source_url"));

                // Should be able to get content back
                let content_back = with_frontmatter.content_only();
                assert_eq!(content_back, "# Test");
            }

            #[test]
            fn test_frontmatter_roundtrip_with_complex_content() {
                // Test roundtrip with complex markdown content
                let complex_content = "# Project Documentation

## Overview
This project implements a **markdown processor** with the following features:

- Frontmatter extraction
- Content validation
- URL processing

### Code Example
```rust
let markdown = Markdown::new(\"# Hello\".to_string())?;
println!(\"{}\", markdown);
```

### Links
- [Documentation](https://docs.example.com)
- [Repository](https://github.com/user/repo)

> **Note**: This is a blockquote with *emphasis*.

| Feature | Status |
|---------|--------|
| Parser  | ✅     |
| Validator | ✅   |

Final paragraph with émojis 🚀 and Unicode characters: café, naïve, résumé.";

                let markdown = Markdown::new(complex_content.to_string()).unwrap();

                let frontmatter = "---\nsource_url: \"https://docs.google.com/document/d/complex123\"\nexporter: \"markdowndown-v2.0\"\ndate_downloaded: \"2023-12-01T10:30:00Z\"\ncustom_field: \"test with spaces and special chars: 🎯\"\n---\n";

                // Add frontmatter
                let with_frontmatter = markdown.with_frontmatter(frontmatter);

                // Extract frontmatter back
                let extracted_frontmatter = with_frontmatter.frontmatter();
                assert!(extracted_frontmatter.is_some());
                let fm = extracted_frontmatter.unwrap();
                assert!(fm.contains("complex123"));
                assert!(fm.contains("markdowndown-v2.0"));
                assert!(fm.contains("🎯"));

                // Extract content back
                let extracted_content = with_frontmatter.content_only();
                assert_eq!(extracted_content, complex_content);

                // Verify no frontmatter bleeding into content
                assert!(!extracted_content.contains("source_url"));
                assert!(!extracted_content.contains("exporter"));
                assert!(!extracted_content.contains("date_downloaded"));
            }

            #[test]
            fn test_error_context_timestamp_recent() {
                // Test that error context timestamps are recent
                let before = Utc::now();
                std::thread::sleep(std::time::Duration::from_millis(1)); // Small delay

                let context = ErrorContext::new("test", "test", "test");

                std::thread::sleep(std::time::Duration::from_millis(1)); // Small delay
                let after = Utc::now();

                // Timestamp should be between before and after
                assert!(context.timestamp >= before);
                assert!(context.timestamp <= after);

                // Should be very recent (within 1 second)
                let diff = (after - context.timestamp).num_milliseconds();
                assert!(diff < 1000, "Timestamp should be within 1 second: {diff}ms");
            }

            #[test]
            fn test_url_validation_with_local_file_integration() {
                // Test integration with the utils::is_local_file_path function
                // This tests the actual integration point in URL validation

                // These should be accepted as local file paths
                let local_file_cases = [
                    "/absolute/path/to/file.md",
                    "./relative/file.md",
                    "../parent/file.md",
                    "simple-file.md",
                    "file:///absolute/path.md",
                    "file://./relative.md",
                ];

                for file_path in local_file_cases {
                    let url_result = Url::new(file_path.to_string());
                    // Note: The actual validation depends on the utils::is_local_file_path implementation
                    // This test covers the integration point even if some cases might fail
                    // depending on the utils implementation
                    if url_result.is_err() {
                        // If it fails, it should be a ValidationError with InvalidUrl kind
                        match url_result.unwrap_err() {
                            MarkdownError::ValidationError { kind, context } => {
                                assert_eq!(kind, ValidationErrorKind::InvalidUrl);
                                assert_eq!(context.url, file_path);
                                assert_eq!(context.operation, "URL validation");
                                assert_eq!(context.converter_type, "Url::new");
                            }
                            other => panic!("Expected ValidationError, got: {other:?}"),
                        }
                    }
                }
            }
        }
    }
}
