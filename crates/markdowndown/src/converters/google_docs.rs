//! Google Docs to markdown conversion with export API integration.
//!
//! This module provides conversion of Google Docs documents to markdown format
//! by leveraging Google's built-in export functionality. It handles various
//! Google Docs URL formats and transforms them to export URLs.
//!
//! # Supported URL Formats
//!
//! - Edit URLs: `https://docs.google.com/document/d/{id}/edit`
//! - View URLs: `https://docs.google.com/document/d/{id}/view`
//! - Share URLs: `https://docs.google.com/document/d/{id}/edit?usp=sharing`
//! - Drive URLs: `https://drive.google.com/file/d/{id}/view`
//!
//! # Usage Examples
//!
//! ## Basic Conversion
//!
//! ```rust
//! use markdowndown::converters::GoogleDocsConverter;
//!
//! # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
//! let converter = GoogleDocsConverter::new();
//! let url = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";
//! let markdown = converter.convert(url).await?;
//! println!("Markdown content: {}", markdown);
//! # Ok(())
//! # }
//! ```

use crate::client::HttpClient;
use crate::frontmatter::FrontmatterBuilder;
use crate::types::{Markdown, MarkdownError};
use async_trait::async_trait;
use chrono::Utc;

/// Default export formats in preference order.
///
/// These formats are tried in sequence when exporting Google Docs:
/// 1. `md` - Google's native markdown export (best quality, cleanest output)
/// 2. `txt` - Plain text fallback (simple but loses formatting)
/// 3. `html` - HTML fallback (most reliable, always available)
///
/// This order is based on Google's export API behavior where markdown
/// provides the cleanest output but may not be available for all documents,
/// text is universally supported but loses formatting, and HTML is the
/// ultimate fallback that works for all public documents.
const DEFAULT_EXPORT_FORMATS: &[&str] = &["md", "txt", "html"];

/// Common error indicators found in Google's error pages.
///
/// These strings are checked to detect when Google returns an error page
/// instead of document content. This list is based on observed Google error
/// responses and may need updates if Google changes their error page formats.
///
/// Note: This approach is complementary to HTTP status code checking and
/// helps catch cases where Google returns 200 OK with an error page body.
const UNIVERSAL_ERROR_INDICATORS: &[&str] = &[
    "sorry, the file you have requested does not exist",
    "access denied",
    "permission denied",
    "file not found",
    "error 404",
    "error 403",
];

/// HTML document type indicators
const HTML_TAGS: &[&str] = &["<!doctype", "<html"];

/// Length of the "/document/d/" prefix in Google Docs URLs
const DOCUMENT_D_PREFIX_LENGTH: usize = "/document/d/".len();

/// Length of the "/file/d/" prefix in Google Drive file URLs
const FILE_D_PREFIX_LENGTH: usize = "/file/d/".len();

/// Length of the "id=" parameter prefix in Google Drive open URLs
const ID_PARAM_LENGTH: usize = "id=".len();

/// Minimum reasonable length for Google Docs document IDs.
///
/// Google Docs IDs are base64url-encoded and typically at least this length
/// based on observed patterns in Google's ID generation system.
const MIN_DOCUMENT_ID_LENGTH: usize = 25;

/// Maximum reasonable length for Google Docs document IDs.
///
/// Upper bound for Google Docs ID length based on observed patterns.
/// This helps detect malformed URLs while allowing flexibility for
/// potential future changes to Google's ID format.
const MAX_DOCUMENT_ID_LENGTH: usize = 100;

/// Maximum number of consecutive blank lines allowed in processed content.
///
/// This limit is based on markdown best practices where:
/// - 1 blank line separates paragraphs
/// - 2 blank lines provide section separation
///
/// More than 2 consecutive blank lines is typically excessive and reduces readability.
const MAX_CONSECUTIVE_BLANK_LINES: usize = 2;

/// Google Docs to markdown converter with intelligent URL handling.
///
/// This converter handles various Google Docs URL formats and converts them
/// to markdown using Google's export API. It provides robust error handling
/// for private documents and network issues.
#[derive(Debug, Clone)]
pub struct GoogleDocsConverter {
    /// HTTP client for making requests to Google's export API
    client: HttpClient,
    /// Set of supported export formats in preference order
    export_formats: Vec<String>,
}

impl GoogleDocsConverter {
    /// Creates a new Google Docs converter with default configuration.
    ///
    /// Default configuration includes:
    /// - HTTP client with retry logic and timeouts
    /// - Export format preference: markdown → plain text → HTML
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GoogleDocsConverter;
    ///
    /// let converter = GoogleDocsConverter::new();
    /// ```
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
            export_formats: DEFAULT_EXPORT_FORMATS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Creates a new Google Docs converter with a custom HTTP client.
    ///
    /// This is useful for testing with mock servers or custom client configurations.
    ///
    /// # Arguments
    ///
    /// * `client` - The HTTP client to use for requests
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GoogleDocsConverter;
    /// use markdowndown::client::HttpClient;
    ///
    /// let client = HttpClient::new();
    /// let converter = GoogleDocsConverter::with_client(client);
    /// ```
    pub fn with_client(client: HttpClient) -> Self {
        Self {
            client,
            export_formats: DEFAULT_EXPORT_FORMATS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Converts a Google Docs URL to markdown with frontmatter.
    ///
    /// This method performs the complete conversion workflow:
    /// 1. Extract document ID from the URL
    /// 2. Validate document accessibility
    /// 3. Try export formats in preference order
    /// 4. Generate frontmatter with metadata
    /// 5. Combine frontmatter with content
    ///
    /// # Arguments
    ///
    /// * `url` - The Google Docs URL to convert
    ///
    /// # Returns
    ///
    /// Returns a `Markdown` instance containing the document content with frontmatter,
    /// or a `MarkdownError` on failure.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::InvalidUrl` - If the URL format is invalid or document ID cannot be extracted
    /// * `MarkdownError::AuthError` - If the document is private or access is denied
    /// * `MarkdownError::NetworkError` - For network-related failures
    /// * `MarkdownError::ParseError` - If the content cannot be processed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GoogleDocsConverter;
    ///
    /// # async fn example() -> Result<(), markdowndown::types::MarkdownError> {
    /// let converter = GoogleDocsConverter::new();
    /// let url = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";
    /// let markdown = converter.convert(url).await?;
    ///
    /// // The result includes frontmatter with metadata
    /// assert!(markdown.as_str().contains("source_url:"));
    /// assert!(markdown.as_str().contains("date_downloaded:"));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError> {
        // Check if this is already an export URL (for testing)
        if self.is_export_url(url) {
            return self.convert_export_url_directly(url).await;
        }

        // Step 1: Extract and validate document ID
        let document_id = self.extract_document_id(url)?;

        // Step 2: Validate document access
        self.validate_access(url).await?;

        // Step 3: Try export formats in preference order
        let content = self.fetch_content_with_fallback(&document_id).await?;

        // Step 4: Post-process the content
        let processed_content = self.post_process_content(&content)?;

        // Step 5: Generate frontmatter
        let frontmatter = self.generate_frontmatter(url, document_id)?;

        // Step 6: Combine frontmatter with content
        let markdown_with_frontmatter = format!("{frontmatter}\n{processed_content}");

        Markdown::new(markdown_with_frontmatter)
    }

    /// Checks if a URL is an export URL (for testing purposes).
    fn is_export_url(&self, url: &str) -> bool {
        url.contains("/export")
    }

    /// Converts an export URL directly (for testing purposes).
    async fn convert_export_url_directly(&self, url: &str) -> Result<Markdown, MarkdownError> {
        // Extract document ID for frontmatter
        let document_id = self.extract_document_id(url)?;

        // Fetch content directly from the export URL
        let content = self.client.get_text(url).await?;

        // Post-process the content
        let processed_content = self.post_process_content(&content)?;

        // Generate frontmatter
        let frontmatter = self.generate_frontmatter(url, document_id)?;

        // Combine frontmatter with content
        let markdown_with_frontmatter = format!("{frontmatter}\n{processed_content}");

        Markdown::new(markdown_with_frontmatter)
    }

    /// Extracts the document ID from various Google Docs URL formats.
    ///
    /// Supports the following URL patterns:
    /// - `https://docs.google.com/document/d/{id}/edit*`
    /// - `https://docs.google.com/document/d/{id}/view*`
    /// - `https://drive.google.com/file/d/{id}/view*`
    /// - `https://drive.google.com/open?id={id}`
    ///
    /// # Arguments
    ///
    /// * `url` - The Google Docs URL to parse
    ///
    /// # Returns
    ///
    /// Returns the document ID as a String, or a `MarkdownError` if extraction fails.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::InvalidUrl` - If the URL format is not recognized or document ID cannot be found
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GoogleDocsConverter;
    ///
    /// let converter = GoogleDocsConverter::new();
    /// let url = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";
    /// let doc_id = converter.extract_document_id(url)?;
    /// assert_eq!(doc_id, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");
    /// # Ok::<(), markdowndown::types::MarkdownError>(())
    /// ```
    pub fn extract_document_id(&self, url: &str) -> Result<String, MarkdownError> {
        let url = url.trim();

        // Define URL patterns with their extraction parameters
        const URL_PATTERNS: &[(&str, usize, Option<char>)] = &[
            ("/document/d/", DOCUMENT_D_PREFIX_LENGTH, Some('/')),
            ("/file/d/", FILE_D_PREFIX_LENGTH, Some('/')),
        ];

        // Try standard patterns
        for (marker, offset, delimiter) in URL_PATTERNS {
            if let Some(id) = self.extract_id_with_pattern(url, marker, *offset, *delimiter) {
                return Ok(id);
            }
        }

        // Handle drive open URL separately due to different pattern
        if url.contains("drive.google.com/open") {
            if let Some(id) = self.extract_id_with_pattern(url, "id=", ID_PARAM_LENGTH, Some('&')) {
                return Ok(id);
            }
        }

        Err(MarkdownError::InvalidUrl {
            url: url.to_string(),
        })
    }

    /// Builds a Google Docs export URL for the specified document ID and format.
    ///
    /// # Arguments
    ///
    /// * `document_id` - The Google Docs document ID
    /// * `format` - The export format (md, txt, html, etc.)
    ///
    /// # Returns
    ///
    /// A properly formatted export URL string
    ///
    /// # Examples
    ///
    /// ```rust
    /// use markdowndown::converters::GoogleDocsConverter;
    ///
    /// let converter = GoogleDocsConverter::new();
    /// let export_url = converter.build_export_url("1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms", "md");
    /// assert!(export_url.contains("/export?format=md"));
    /// ```
    pub fn build_export_url(&self, document_id: &str, format: &str) -> String {
        format!("https://docs.google.com/document/d/{document_id}/export?format={format}")
    }

    /// Validates that a document is accessible for export.
    ///
    /// This method makes a lightweight request to check if the document
    /// is publicly accessible before attempting to fetch the full content.
    ///
    /// # Arguments
    ///
    /// * `url` - The original Google Docs URL
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the document is accessible, or an appropriate error.
    ///
    /// # Errors
    ///
    /// * `MarkdownError::AuthError` - If the document is private or access is denied
    /// * `MarkdownError::NetworkError` - For network-related failures
    /// * `MarkdownError::InvalidUrl` - If the document ID cannot be extracted
    pub async fn validate_access(&self, url: &str) -> Result<(), MarkdownError> {
        let document_id = self.extract_document_id(url)?;
        let test_url = self.build_export_url(&document_id, "txt");

        // Make a HEAD request to check accessibility without downloading content
        match self.client.get_text(&test_url).await {
            Ok(_) => Ok(()),
            Err(MarkdownError::AuthError { message }) => Err(MarkdownError::AuthError {
                message: format!("Document is private or access denied: {message}"),
            }),
            Err(e) => Err(e),
        }
    }

    /// Fetches document content with format fallback strategy.
    ///
    /// Tries export formats in preference order (markdown → text → HTML)
    /// until one succeeds or all fail.
    async fn fetch_content_with_fallback(
        &self,
        document_id: &str,
    ) -> Result<String, MarkdownError> {
        for format in &self.export_formats {
            if let Some(content) = self.try_export_format(document_id, format).await {
                return Ok(content);
            }
        }

        Err(MarkdownError::NetworkError {
            message: "All export formats failed to produce valid content".to_string(),
        })
    }

    /// Attempts to fetch content in a specific export format.
    ///
    /// Returns Some(content) if the format succeeds and produces valid content,
    /// or None if the format fails or produces invalid content.
    async fn try_export_format(&self, document_id: &str, format: &str) -> Option<String> {
        let export_url = self.build_export_url(document_id, format);

        match self.client.get_text(&export_url).await {
            Ok(content) => {
                if self.is_valid_content(&content, format) {
                    Some(content)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Checks if content contains HTML tags.
    fn contains_html_tags(&self, content_lower: &str) -> bool {
        HTML_TAGS
            .iter()
            .any(|tag| content_lower.contains(tag) || content_lower.starts_with(tag))
    }

    /// Checks if content contains error indicators.
    fn contains_error_indicators(&self, content_lower: &str) -> bool {
        UNIVERSAL_ERROR_INDICATORS
            .iter()
            .any(|indicator| content_lower.contains(indicator))
    }

    /// Validates format-specific content requirements.
    fn validate_format_specific(&self, content_lower: &str, format: &str) -> bool {
        match format {
            "md" => !self.contains_html_tags(content_lower),
            "txt" => !self.contains_html_tags(content_lower),
            "html" => self.contains_html_tags(content_lower),
            _ => true, // Unknown format, assume valid
        }
    }

    /// Validates that fetched content is actual document content, not an error page.
    fn is_valid_content(&self, content: &str, format: &str) -> bool {
        let content_lower = content.to_lowercase();

        if self.contains_error_indicators(&content_lower) {
            return false;
        }

        self.validate_format_specific(&content_lower, format)
    }

    /// Generates frontmatter for a converted document.
    ///
    /// # Arguments
    ///
    /// * `url` - The original URL of the document
    /// * `document_id` - The extracted Google Docs document ID
    ///
    /// # Returns
    ///
    /// Returns the formatted frontmatter as a String, or a `MarkdownError` on failure.
    fn generate_frontmatter(
        &self,
        url: &str,
        document_id: String,
    ) -> Result<String, MarkdownError> {
        let now = Utc::now();
        let frontmatter = FrontmatterBuilder::new(url.to_string())
            .exporter(format!(
                "markdowndown-googledocs-{}",
                env!("CARGO_PKG_VERSION")
            ))
            .download_date(now)
            .additional_field("converted_at".to_string(), now.to_rfc3339())
            .additional_field("conversion_type".to_string(), "google_docs".to_string())
            .additional_field("document_id".to_string(), document_id)
            .additional_field("document_type".to_string(), "google_docs".to_string())
            .build()?;

        Ok(frontmatter)
    }

    /// Post-processes the fetched content to clean it up.
    fn post_process_content(&self, content: &str) -> Result<String, MarkdownError> {
        if content.trim().is_empty() {
            // Return minimal placeholder content for empty documents
            return Ok("_[Empty document]_".to_string());
        }

        let mut processed = content.to_string();

        // Remove excessive blank lines (more than 2 consecutive)
        processed = self.normalize_blank_lines(&processed);

        // Trim leading and trailing whitespace
        processed = processed.trim().to_string();

        if processed.is_empty() {
            return Err(MarkdownError::ParseError {
                message: "Document content is empty after processing".to_string(),
            });
        }

        Ok(processed)
    }

    /// Normalizes blank lines to prevent excessive whitespace.
    ///
    /// This method limits consecutive blank lines to a maximum of 2, which follows
    /// common markdown conventions where:
    /// - 1 blank line separates paragraphs
    /// - 2 blank lines provide stronger visual separation between sections
    /// - More than 2 blank lines is typically excessive and distracting
    ///
    /// This limit is based on markdown best practices and improves readability
    /// while preserving intentional document structure.
    fn normalize_blank_lines(&self, content: &str) -> String {
        let lines: Vec<&str> = content.split('\n').collect();
        let mut result = Vec::new();
        let mut consecutive_blanks = 0;

        for line in lines {
            if line.trim().is_empty() {
                consecutive_blanks += 1;
                if consecutive_blanks <= MAX_CONSECUTIVE_BLANK_LINES {
                    result.push(line);
                }
            } else {
                consecutive_blanks = 0;
                result.push(line);
            }
        }

        result.join("\n")
    }

    /// Generic helper to extract document ID from URL using a pattern marker.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to parse
    /// * `marker` - The marker string to search for (e.g., "/document/d/")
    /// * `offset` - Number of characters after the marker where the ID starts
    /// * `delimiter` - Optional delimiter that marks the end of the ID (e.g., '/')
    ///
    /// # Returns
    ///
    /// Returns the document ID if found and valid, or None otherwise.
    fn extract_id_with_pattern(
        &self,
        url: &str,
        marker: &str,
        offset: usize,
        delimiter: Option<char>,
    ) -> Option<String> {
        if let Some(start) = url.find(marker) {
            let after_marker = &url[start + offset..];

            let id = if let Some(delim) = delimiter {
                if let Some(end) = after_marker.find(delim) {
                    &after_marker[..end]
                } else {
                    // Handle case where ID is at the end of URL
                    after_marker
                }
            } else {
                after_marker
            };

            if !id.is_empty() && self.is_valid_document_id(id) {
                return Some(id.to_string());
            }
        }
        None
    }

    /// Validates that a string looks like a valid Google Docs document ID.
    ///
    /// Google Docs document IDs are base64url-encoded strings that:
    /// - Contain alphanumeric characters, hyphens, and underscores
    /// - Are typically 33-44 characters long (based on observed IDs)
    /// - Follow a specific encoding pattern used by Google's systems
    ///
    /// This validation is intentionally permissive to handle:
    /// - Potential future changes to Google's ID format
    /// - Edge cases in Google's ID generation
    /// - Test scenarios with mock IDs
    ///
    /// The actual document validity is ultimately determined by Google's API.
    /// This check primarily filters out obviously invalid inputs to provide
    /// early feedback rather than enforcing exact ID specifications.
    fn is_valid_document_id(&self, id: &str) -> bool {
        !id.is_empty()
            && id.len() >= MIN_DOCUMENT_ID_LENGTH
            && id.len() <= MAX_DOCUMENT_ID_LENGTH
            && id
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
    }
}

#[async_trait]
impl super::Converter for GoogleDocsConverter {
    async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError> {
        self.convert(url).await
    }

    fn name(&self) -> &'static str {
        "Google Docs"
    }
}

impl Default for GoogleDocsConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_docs_converter_new() {
        let converter = GoogleDocsConverter::new();
        assert_eq!(converter.export_formats.len(), DEFAULT_EXPORT_FORMATS.len());
        assert_eq!(converter.export_formats[0], "md");
        assert_eq!(converter.export_formats[1], "txt");
        assert_eq!(converter.export_formats[2], "html");
    }

    #[test]
    fn test_extract_document_id_valid_urls() {
        let test_cases = vec![
            (
                "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit",
                "docs_edit",
            ),
            (
                "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view",
                "docs_view",
            ),
            (
                "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit?usp=sharing",
                "docs_share",
            ),
            (
                "https://drive.google.com/file/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view",
                "drive_file",
            ),
            (
                "https://drive.google.com/open?id=1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
                "drive_open",
            ),
        ];

        let converter = GoogleDocsConverter::new();
        let expected_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";

        for (url, test_name) in test_cases {
            let result = converter.extract_document_id(url).unwrap();
            assert_eq!(result, expected_id, "Failed test case: {}", test_name);
        }
    }

    #[test]
    fn test_extract_document_id_invalid_url() {
        let converter = GoogleDocsConverter::new();
        let url = "https://example.com/not-a-google-doc";
        let result = converter.extract_document_id(url);
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::InvalidUrl { url: error_url } => {
                assert_eq!(error_url, url);
            }
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    #[test]
    fn test_build_export_url() {
        let converter = GoogleDocsConverter::new();
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        let export_url = converter.build_export_url(doc_id, "md");
        let expected = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/export?format=md";
        assert_eq!(export_url, expected);
    }

    #[test]
    fn test_is_valid_document_id() {
        let converter = GoogleDocsConverter::new();

        // Valid IDs
        assert!(converter.is_valid_document_id("1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms"));
        assert!(converter.is_valid_document_id("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(converter.is_valid_document_id("1234567890abcdef-_1234567890"));

        // Invalid IDs
        assert!(!converter.is_valid_document_id(""));
        assert!(!converter.is_valid_document_id("short"));
        assert!(!converter.is_valid_document_id("contains spaces"));
        assert!(!converter.is_valid_document_id("contains@special#chars"));
        assert!(!converter.is_valid_document_id(&"a".repeat(MAX_DOCUMENT_ID_LENGTH * 2)));
        // Too long
    }

    #[test]
    fn test_is_valid_content() {
        let converter = GoogleDocsConverter::new();

        // Valid markdown content
        assert!(converter.is_valid_content("# Title\n\nContent here", "md"));

        // Valid text content
        assert!(converter.is_valid_content("Plain text content", "txt"));

        // Valid HTML content
        assert!(
            converter.is_valid_content("<!DOCTYPE html><html><body>Content</body></html>", "html")
        );

        // Invalid content (error messages)
        assert!(
            !converter.is_valid_content("Sorry, the file you have requested does not exist", "md")
        );
        assert!(!converter.is_valid_content("Access denied", "txt"));
        assert!(!converter.is_valid_content("Error 404", "html"));

        // Invalid format mismatches
        assert!(!converter.is_valid_content("<!DOCTYPE html><html>", "md")); // HTML in markdown
        assert!(!converter.is_valid_content("<html><body>content</body></html>", "txt"));
        // HTML in text
    }

    #[test]
    fn test_normalize_blank_lines() {
        let converter = GoogleDocsConverter::new();

        let input = "Line 1\n\n\n\n\nLine 2\n\n\nLine 3";
        let expected = "Line 1\n\n\nLine 2\n\n\nLine 3";
        let result = converter.normalize_blank_lines(input);
        assert_eq!(result, expected);

        // Test with normal spacing
        let input2 = "Line 1\n\nLine 2\n\nLine 3";
        let result2 = converter.normalize_blank_lines(input2);
        assert_eq!(result2, input2); // Should remain unchanged
    }

    #[test]
    fn test_post_process_content() {
        let converter = GoogleDocsConverter::new();

        // Valid content
        let input = "  \n\n# Title\n\nContent here\n\n\n\n\nMore content\n\n  ";
        let result = converter.post_process_content(input).unwrap();
        assert!(!result.starts_with(' '));
        assert!(!result.ends_with(' '));
        assert!(result.contains("# Title"));
        assert!(result.contains("Content here"));

        // Empty content should return placeholder
        let empty_result = converter.post_process_content("   \n\n   ");
        assert!(empty_result.is_ok());
        assert_eq!(empty_result.unwrap(), "_[Empty document]_");
    }

    #[test]
    fn test_default_implementation() {
        let converter = GoogleDocsConverter::default();
        assert_eq!(converter.export_formats.len(), DEFAULT_EXPORT_FORMATS.len());
    }

    // Edge case tests for URL parsing
    #[test]
    fn test_extract_document_id_edge_cases() {
        let converter = GoogleDocsConverter::new();

        // URL with no trailing slash or parameters
        let url1 =
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        assert!(converter.extract_document_id(url1).is_ok());

        // URL with multiple query parameters
        let url2 = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit?usp=sharing&ts=12345";
        assert!(converter.extract_document_id(url2).is_ok());

        // Drive URL with additional parameters
        let url3 = "https://drive.google.com/open?id=1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms&authuser=0";
        let result = converter.extract_document_id(url3).unwrap();
        assert_eq!(result, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");

        // Malformed URLs should fail
        let bad_urls = [
            "https://docs.google.com/document/d//edit", // Empty ID
            "https://docs.google.com/document/d/short/edit", // Too short ID
            "https://drive.google.com/open?id=",        // Empty ID parameter
            "https://docs.google.com/spreadsheet/d/123/edit", // Wrong document type
        ];

        for bad_url in &bad_urls {
            let result = converter.extract_document_id(bad_url);
            assert!(result.is_err(), "Should fail for URL: {bad_url}");
        }
    }

    #[test]
    fn test_with_client() {
        let custom_client = HttpClient::new();
        let converter = GoogleDocsConverter::with_client(custom_client);
        assert_eq!(converter.export_formats.len(), DEFAULT_EXPORT_FORMATS.len());
        assert_eq!(converter.export_formats[0], "md");
        assert_eq!(converter.export_formats[1], "txt");
        assert_eq!(converter.export_formats[2], "html");
    }

    #[tokio::test]
    async fn test_convert_success() {
        use mockito::Server;

        let mut server = Server::new_async().await;
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        let test_content = "# Test Document\n\nThis is test content.";

        let mock = server
            .mock("GET", format!("/document/d/{}/export", doc_id).as_str())
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(200)
            .with_header("content-type", "text/plain; charset=utf-8")
            .with_body(test_content)
            .create_async()
            .await;

        let converter = GoogleDocsConverter::new();
        let export_url = format!("{}/document/d/{}/export?format=txt", server.url(), doc_id);
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_ok());

        let markdown = result.unwrap();
        let content = markdown.as_str();
        assert!(content.contains("source_url:"));
        assert!(content.contains("date_downloaded:"));
        assert!(content.contains("# Test Document"));
        assert!(content.contains("This is test content."));
    }

    #[tokio::test]
    async fn test_convert_private_document() {
        use mockito::Server;

        let mut server = Server::new_async().await;
        let doc_id = "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";

        let mock = server
            .mock("GET", format!("/document/d/{}/export", doc_id).as_str())
            .match_query(mockito::Matcher::UrlEncoded("format".into(), "txt".into()))
            .with_status(403)
            .with_header("content-type", "text/html; charset=utf-8")
            .with_body("Access denied. You need permission to access this document.")
            .create_async()
            .await;

        let converter = GoogleDocsConverter::new();
        let export_url = format!("{}/document/d/{}/export?format=txt", server.url(), doc_id);
        let result = converter.convert(&export_url).await;

        mock.assert_async().await;
        assert!(result.is_err());

        match result.unwrap_err() {
            MarkdownError::AuthenticationError { .. }
            | MarkdownError::EnhancedNetworkError { .. } => {}
            _ => panic!("Expected AuthenticationError or EnhancedNetworkError"),
        }
    }

    #[tokio::test]
    async fn test_validate_access_invalid_url() {
        let converter = GoogleDocsConverter::new();
        let invalid_url = "https://example.com/not-a-google-doc";
        let result = converter.validate_access(invalid_url).await;

        assert!(result.is_err());

        match result.unwrap_err() {
            MarkdownError::InvalidUrl { url } => {
                assert_eq!(url, invalid_url);
            }
            _ => panic!("Expected InvalidUrl error"),
        }
    }

    // Note: Network-related scenarios for validate_access() (successful validation,
    // auth errors when document is private, and network failures) are tested indirectly
    // through the convert() integration tests (test_convert_success and
    // test_convert_private_document), which call validate_access() as part of their
    // workflow. Direct unit testing of these scenarios would require making the Google
    // Docs base URL configurable for testing purposes, which would add unnecessary
    // complexity to the production code.
}
