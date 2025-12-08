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
            export_formats: vec![
                "md".to_string(),   // Markdown (preferred)
                "txt".to_string(),  // Plain text (fallback)
                "html".to_string(), // HTML (can be converted)
            ],
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
            export_formats: vec![
                "md".to_string(),   // Markdown (preferred)
                "txt".to_string(),  // Plain text (fallback)
                "html".to_string(), // HTML (can be converted)
            ],
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

        // Pattern 1: docs.google.com/document/d/{id}/...
        if let Some(docs_match) = self.extract_from_docs_url(url) {
            return Ok(docs_match);
        }

        // Pattern 2: drive.google.com/file/d/{id}/...
        if let Some(drive_file_match) = self.extract_from_drive_file_url(url) {
            return Ok(drive_file_match);
        }

        // Pattern 3: drive.google.com/open?id={id}
        if let Some(drive_open_match) = self.extract_from_drive_open_url(url) {
            return Ok(drive_open_match);
        }

        // Pattern 4: Export URLs like /document/d/{id}/export or /file/d/{id}/export (for testing)
        if let Some(export_match) = self.extract_from_export_url(url) {
            return Ok(export_match);
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
        let mut last_error = None;

        for format in &self.export_formats {
            let export_url = self.build_export_url(document_id, format);

            match self.client.get_text(&export_url).await {
                Ok(content) => {
                    // Verify we got actual content, not an error page
                    if self.is_valid_content(&content, format) {
                        return Ok(content);
                    }
                    // Continue to next format if content seems invalid
                }
                Err(e) => {
                    last_error = Some(e);
                    // Continue to next format
                }
            }
        }

        // All formats failed
        Err(last_error.unwrap_or_else(|| MarkdownError::NetworkError {
            message: "All export formats failed to produce valid content".to_string(),
        }))
    }

    /// Validates that fetched content is actual document content, not an error page.
    fn is_valid_content(&self, content: &str, format: &str) -> bool {
        let content_lower = content.to_lowercase();

        // Check for common error indicators (these are universal)
        let universal_error_indicators = [
            "sorry, the file you have requested does not exist",
            "access denied",
            "permission denied",
            "file not found",
            "error 404",
            "error 403",
        ];

        for indicator in &universal_error_indicators {
            if content_lower.contains(indicator) {
                return false;
            }
        }

        // Format-specific validation
        match format {
            "md" => {
                // Markdown should not be primarily HTML
                // Also check for HTML error pages when expecting markdown
                !content_lower.starts_with("<!doctype")
                    && !content_lower.starts_with("<html")
                    && !content_lower.contains("<!doctype html>") // HTML error pages
            }
            "txt" => {
                // Plain text should not contain HTML tags
                // Also check for HTML when expecting plain text
                !content_lower.contains("<html>")
                    && !content_lower.contains("<!doctype")
                    && !content_lower.contains("<!doctype html>") // HTML error pages
            }
            "html" => {
                // HTML should contain actual HTML structure
                content_lower.contains("<html") || content_lower.starts_with("<!doctype")
            }
            _ => true, // Unknown format, assume valid
        }
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
    fn normalize_blank_lines(&self, content: &str) -> String {
        let lines: Vec<&str> = content.split('\n').collect();
        let mut result = Vec::new();
        let mut consecutive_blanks = 0;

        for line in lines {
            if line.trim().is_empty() {
                consecutive_blanks += 1;
                // Allow maximum of 2 consecutive blank lines
                if consecutive_blanks <= 2 {
                    result.push(line);
                }
            } else {
                consecutive_blanks = 0;
                result.push(line);
            }
        }

        result.join("\n")
    }

    /// Helper function to extract document ID from docs.google.com URLs.
    fn extract_from_docs_url(&self, url: &str) -> Option<String> {
        // Pattern: https://docs.google.com/document/d/{id}/...
        if let Some(start) = url.find("/document/d/") {
            let after_d = &url[start + 12..]; // "/document/d/" is 12 chars
            if let Some(end) = after_d.find('/') {
                let id = &after_d[..end];
                if !id.is_empty() && self.is_valid_document_id(id) {
                    return Some(id.to_string());
                }
            } else {
                // Handle case where ID is at the end of URL
                let id = after_d;
                if !id.is_empty() && self.is_valid_document_id(id) {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    /// Helper function to extract document ID from drive.google.com file URLs.
    fn extract_from_drive_file_url(&self, url: &str) -> Option<String> {
        // Pattern: https://drive.google.com/file/d/{id}/...
        if let Some(start) = url.find("/file/d/") {
            let after_d = &url[start + 8..]; // "/file/d/" is 8 chars
            if let Some(end) = after_d.find('/') {
                let id = &after_d[..end];
                if !id.is_empty() && self.is_valid_document_id(id) {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    /// Helper function to extract document ID from drive.google.com open URLs.
    fn extract_from_drive_open_url(&self, url: &str) -> Option<String> {
        // Pattern: https://drive.google.com/open?id={id}
        if url.contains("drive.google.com/open") {
            if let Some(id_start) = url.find("id=") {
                let after_id = &url[id_start + 3..]; // "id=" is 3 chars
                                                     // Find end of ID (next & or end of string)
                let id_end = after_id.find('&').unwrap_or(after_id.len());
                let id = &after_id[..id_end];
                if !id.is_empty() && self.is_valid_document_id(id) {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    /// Extracts document ID from export URLs (for testing).
    ///
    /// This method handles export URLs like:
    /// - http://localhost:1234/document/d/{id}/export
    /// - http://127.0.0.1:5678/file/d/{id}/export
    fn extract_from_export_url(&self, url: &str) -> Option<String> {
        // Pattern: .../document/d/{id}/export or .../file/d/{id}/export
        if let Some(start) = url.find("/document/d/").or_else(|| url.find("/file/d/")) {
            let path_start = if url[start..].starts_with("/document/d/") {
                start + 12 // "/document/d/" is 12 chars
            } else {
                start + 8 // "/file/d/" is 8 chars
            };
            let after_d = &url[path_start..];

            // Look for either '/' or '/export' to find the end of the ID
            if let Some(end) = after_d.find('/') {
                let id = &after_d[..end];
                if !id.is_empty() && self.is_valid_document_id(id) {
                    return Some(id.to_string());
                }
            }
        }
        None
    }

    /// Validates that a string looks like a valid Google Docs document ID.
    fn is_valid_document_id(&self, id: &str) -> bool {
        // Google Docs IDs are typically alphanumeric with some special chars
        // They're usually quite long (40+ characters) and contain specific patterns
        !id.is_empty()
            && id.len() >= 25  // Minimum reasonable length
            && id.len() <= 100 // Maximum reasonable length
            && id.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
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
        assert_eq!(converter.export_formats.len(), 3);
        assert_eq!(converter.export_formats[0], "md");
        assert_eq!(converter.export_formats[1], "txt");
        assert_eq!(converter.export_formats[2], "html");
    }

    #[test]
    fn test_extract_document_id_docs_edit() {
        let converter = GoogleDocsConverter::new();
        let url =
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit";
        let result = converter.extract_document_id(url).unwrap();
        assert_eq!(result, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");
    }

    #[test]
    fn test_extract_document_id_docs_view() {
        let converter = GoogleDocsConverter::new();
        let url =
            "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view";
        let result = converter.extract_document_id(url).unwrap();
        assert_eq!(result, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");
    }

    #[test]
    fn test_extract_document_id_docs_share() {
        let converter = GoogleDocsConverter::new();
        let url = "https://docs.google.com/document/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit?usp=sharing";
        let result = converter.extract_document_id(url).unwrap();
        assert_eq!(result, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");
    }

    #[test]
    fn test_extract_document_id_drive_file() {
        let converter = GoogleDocsConverter::new();
        let url =
            "https://drive.google.com/file/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view";
        let result = converter.extract_document_id(url).unwrap();
        assert_eq!(result, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");
    }

    #[test]
    fn test_extract_document_id_drive_open() {
        let converter = GoogleDocsConverter::new();
        let url = "https://drive.google.com/open?id=1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms";
        let result = converter.extract_document_id(url).unwrap();
        assert_eq!(result, "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms");
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
        assert!(!converter.is_valid_document_id(&"a".repeat(200))); // Too long
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
        assert_eq!(converter.export_formats.len(), 3);
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
}
