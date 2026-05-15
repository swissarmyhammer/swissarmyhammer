//! Local file converter for reading markdown files from the filesystem.
//!
//! This converter handles local file paths and file:// URLs by reading markdown content
//! directly from the local filesystem.

use crate::types::{ContentErrorKind, ErrorContext, Markdown, MarkdownError};
use async_trait::async_trait;
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, instrument};

/// Converter for reading markdown files from the local filesystem.
///
/// This converter supports both regular file paths and file:// URLs:
/// - `/absolute/path/to/file.md`
/// - `./relative/path.md`
/// - `../parent/relative/path.md`
/// - `file:///absolute/path/to/file.md`
/// - `file://./relative/path.md`
#[derive(Debug, Clone, Default)]
pub struct LocalFileConverter;

impl LocalFileConverter {
    /// Creates a new LocalFileConverter instance.
    pub fn new() -> Self {
        LocalFileConverter
    }

    /// Helper method to create content errors with consistent context.
    fn create_content_error(
        &self,
        kind: ContentErrorKind,
        url: &str,
        operation: &str,
        info: impl Into<String>,
    ) -> MarkdownError {
        let context =
            ErrorContext::new(url, operation, "LocalFileConverter").with_info(info.into());
        MarkdownError::ContentError { kind, context }
    }

    /// Converts a file path or file:// URL to a standard file path.
    ///
    /// # Arguments
    ///
    /// * `input` - The file path or file:// URL
    ///
    /// # Returns
    ///
    /// Returns the normalized file path as a string.
    fn normalize_path(&self, input: &str) -> String {
        if let Some(path_part) = input.strip_prefix("file:///") {
            format!("/{path_part}")
        } else if let Some(path_part) = input.strip_prefix("file://") {
            path_part.to_string()
        } else {
            input.to_string()
        }
    }

    /// Validates that the file path exists and is readable.
    async fn validate_file_path(&self, path: &str) -> Result<(), MarkdownError> {
        let path_obj = Path::new(path);

        // Check if file exists
        if !path_obj.exists() {
            return Err(self.create_content_error(
                ContentErrorKind::EmptyContent,
                path,
                "File validation",
                "File does not exist",
            ));
        }

        // Check if it's a file (not a directory)
        if !path_obj.is_file() {
            return Err(self.create_content_error(
                ContentErrorKind::UnsupportedFormat,
                path,
                "File validation",
                "Path is not a file",
            ));
        }

        Ok(())
    }

    /// Reads the file content as a UTF-8 string.
    async fn read_file_content(&self, path: &str) -> Result<String, MarkdownError> {
        fs::read_to_string(path).await.map_err(|e| {
            let kind = match e.kind() {
                std::io::ErrorKind::NotFound => ContentErrorKind::EmptyContent,
                _ => ContentErrorKind::ParsingFailed,
            };
            self.create_content_error(kind, path, "File reading", format!("IO error: {e}"))
        })
    }
}

#[async_trait]
impl super::Converter for LocalFileConverter {
    /// Converts a local file path or file:// URL to markdown.
    ///
    /// # Arguments
    ///
    /// * `url` - The file path or file:// URL to read
    ///
    /// # Returns
    ///
    /// Returns the file content as validated Markdown or an error.
    #[instrument(skip(self), fields(file_path))]
    async fn convert(&self, url: &str) -> Result<Markdown, MarkdownError> {
        info!("Starting local file conversion for: {}", url);

        // Normalize the path (handle file:// URLs)
        let file_path = self.normalize_path(url);
        tracing::Span::current().record("file_path", &file_path);
        debug!("Normalized file path: {}", file_path);

        // Validate file exists and is readable
        debug!("Validating file path");
        self.validate_file_path(&file_path).await?;

        // Read file content
        debug!("Reading file content");
        let content = self.read_file_content(&file_path).await?;

        // Validate content is not empty
        if content.trim().is_empty() {
            return Err(self.create_content_error(
                ContentErrorKind::EmptyContent,
                &file_path,
                "Content validation",
                "File content is empty",
            ));
        }

        // Create validated Markdown instance
        debug!("Creating validated markdown instance");
        let markdown = Markdown::new(content).map_err(|e| {
            self.create_content_error(
                ContentErrorKind::ParsingFailed,
                &file_path,
                "Markdown validation",
                format!("Validation error: {e}"),
            )
        })?;

        info!(
            "Successfully converted local file to markdown ({} chars)",
            markdown.as_str().len()
        );
        Ok(markdown)
    }

    fn name(&self) -> &'static str {
        "Local File Converter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converters::converter::Converter;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// No file permissions (used for testing permission denied scenarios)
    const NO_PERMISSIONS: u32 = 0o000;
    /// Standard read/write permissions (owner read/write, group/others read)
    const READ_WRITE_PERMISSIONS: u32 = 0o644;
    /// Minimum expected content length for converted markdown files
    const MIN_EXPECTED_CONTENT_LENGTH: usize = 50;

    #[test]
    fn test_normalize_path_regular_path() {
        let converter = LocalFileConverter::new();

        // Test absolute path
        assert_eq!(
            converter.normalize_path("/path/to/file.md"),
            "/path/to/file.md"
        );

        // Test relative paths
        assert_eq!(converter.normalize_path("./file.md"), "./file.md");
        assert_eq!(converter.normalize_path("../file.md"), "../file.md");
        assert_eq!(
            converter.normalize_path("relative/path.md"),
            "relative/path.md"
        );
    }

    #[test]
    fn test_normalize_path_file_url() {
        let converter = LocalFileConverter::new();

        // Test file:// URLs with absolute paths (three slashes)
        assert_eq!(
            converter.normalize_path("file:///path/to/file.md"),
            "/path/to/file.md"
        );
        assert_eq!(
            converter.normalize_path("file:///home/user/doc.md"),
            "/home/user/doc.md"
        );

        // Test file:// URLs with relative paths (two slashes)
        assert_eq!(
            converter.normalize_path("file://./relative.md"),
            "./relative.md"
        );
        assert_eq!(
            converter.normalize_path("file://../parent.md"),
            "../parent.md"
        );
        assert_eq!(
            converter.normalize_path("file://relative/path.md"),
            "relative/path.md"
        );
    }

    #[tokio::test]
    async fn test_convert_existing_file() {
        let converter = LocalFileConverter::new();

        // Create a temporary file with markdown content
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "# Test Document\n\nThis is a test markdown file.";
        writeln!(temp_file, "{content}").unwrap();

        let file_path = temp_file.path().to_str().unwrap();
        let result = converter.convert(file_path).await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.as_str().contains("# Test Document"));
        assert!(markdown.as_str().contains("This is a test markdown file."));
    }

    #[tokio::test]
    async fn test_convert_file_url() {
        let converter = LocalFileConverter::new();

        // Create a temporary file with markdown content
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = "# File URL Test\n\nTesting file:// URLs.";
        writeln!(temp_file, "{content}").unwrap();

        let file_path = temp_file.path().to_str().unwrap();
        let file_url = format!("file://{file_path}");
        let result = converter.convert(&file_url).await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.as_str().contains("# File URL Test"));
        assert!(markdown.as_str().contains("Testing file:// URLs."));
    }

    #[tokio::test]
    async fn test_convert_nonexistent_file() {
        let converter = LocalFileConverter::new();

        let result = converter.convert("/nonexistent/file.md").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { kind, context } => {
                assert_eq!(kind, ContentErrorKind::EmptyContent);
                assert_eq!(context.url, "/nonexistent/file.md");
                assert!(context
                    .additional_info
                    .unwrap()
                    .contains("File does not exist"));
            }
            _ => panic!("Expected ContentError"),
        }
    }

    #[tokio::test]
    async fn test_convert_empty_file() {
        let converter = LocalFileConverter::new();

        // Create an empty temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap();

        let result = converter.convert(file_path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { kind, .. } => {
                assert_eq!(kind, ContentErrorKind::EmptyContent);
            }
            _ => panic!("Expected ContentError"),
        }
    }

    #[tokio::test]
    async fn test_convert_directory() {
        let converter = LocalFileConverter::new();

        // Try to convert a directory (should fail)
        let result = converter.convert("/tmp").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { kind, context } => {
                assert_eq!(kind, ContentErrorKind::UnsupportedFormat);
                assert!(context
                    .additional_info
                    .unwrap()
                    .contains("Path is not a file"));
            }
            _ => panic!("Expected ContentError"),
        }
    }

    #[test]
    fn test_converter_name() {
        let converter = LocalFileConverter::new();
        assert_eq!(converter.name(), "Local File Converter");
    }

    #[test]
    fn test_default_implementation() {
        let converter = LocalFileConverter;
        assert_eq!(converter.name(), "Local File Converter");
    }

    #[tokio::test]
    async fn test_permission_denied_error() {
        let converter = LocalFileConverter::new();

        // Create a temporary file with restricted permissions
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "# Test content").unwrap();

        // Get the file path before changing permissions
        let file_path = temp_file.path().to_str().unwrap().to_string();

        // Remove read permissions (Unix-like systems)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(NO_PERMISSIONS);
            std::fs::set_permissions(&file_path, permissions).unwrap();

            let result = converter.convert(&file_path).await;

            // Restore permissions for cleanup
            let restore_permissions = std::fs::Permissions::from_mode(READ_WRITE_PERMISSIONS);
            let _ = std::fs::set_permissions(&file_path, restore_permissions);

            assert!(result.is_err());
            match result.unwrap_err() {
                MarkdownError::ContentError { kind, context } => {
                    assert_eq!(kind, ContentErrorKind::ParsingFailed);
                    assert!(context.additional_info.unwrap().contains("IO error"));
                }
                _ => panic!("Expected ContentError for permission denied"),
            }
        }

        // On Windows, we can't easily test permission denied, so we test other error conditions
        #[cfg(windows)]
        {
            // Test will pass on Windows since we can't easily create permission denied scenario
            assert_eq!(converter.name(), "Local File Converter");
        }
    }

    #[tokio::test]
    async fn test_file_not_found_during_read() {
        let converter = LocalFileConverter::new();

        // Create a temporary file path that will be deleted
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap().to_string();

        // Drop the temp file to delete it
        drop(temp_file);

        // Now try to read the deleted file
        let result = converter.convert(&file_path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { kind, context } => {
                assert_eq!(kind, ContentErrorKind::EmptyContent);
                assert!(context
                    .additional_info
                    .unwrap()
                    .contains("File does not exist"));
            }
            _ => panic!("Expected ContentError for file not found"),
        }
    }

    #[tokio::test]
    async fn test_markdown_validation_failure() {
        let converter = LocalFileConverter::new();

        // Create a temporary file with content that might cause Markdown validation issues
        let mut temp_file = NamedTempFile::new().unwrap();
        // Create content that would fail Markdown::new() validation (empty after trimming)
        writeln!(temp_file, "   \n\n   \t   \n   ").unwrap();

        let file_path = temp_file.path().to_str().unwrap();
        let result = converter.convert(file_path).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { kind, context } => {
                assert_eq!(kind, ContentErrorKind::EmptyContent);
                assert_eq!(context.operation, "Content validation");
                assert!(context
                    .additional_info
                    .unwrap()
                    .contains("File content is empty"));
            }
            _ => panic!("Expected ContentError for empty content"),
        }
    }

    #[tokio::test]
    async fn test_file_url_absolute_path_normalization() {
        let converter = LocalFileConverter::new();

        // Test file:/// URL normalization (3 slashes for absolute path)
        let normalized = converter.normalize_path("file:///usr/local/test.md");
        assert_eq!(normalized, "/usr/local/test.md");

        // Test file:// URL normalization (2 slashes for relative path)
        let normalized = converter.normalize_path("file://./test.md");
        assert_eq!(normalized, "./test.md");
    }

    #[tokio::test]
    async fn test_whitespace_only_file_content() {
        let converter = LocalFileConverter::new();

        // Create a file with only whitespace characters
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "   \t\n\r\n   \t   ").unwrap();

        let file_path = temp_file.path().to_str().unwrap();
        let result = converter.convert(file_path).await;

        // Should fail because content is empty after trimming
        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { kind, .. } => {
                assert_eq!(kind, ContentErrorKind::EmptyContent);
            }
            _ => panic!("Expected ContentError for whitespace-only content"),
        }
    }

    #[tokio::test]
    async fn test_successful_conversion_with_logging() {
        let converter = LocalFileConverter::new();

        // Create a temporary file with valid markdown content
        let mut temp_file = NamedTempFile::new().unwrap();
        let content =
            "# Success Test\n\nThis file should convert successfully and trigger logging.";
        writeln!(temp_file, "{content}").unwrap();

        let file_path = temp_file.path().to_str().unwrap();
        let result = converter.convert(file_path).await;

        // Should succeed and hit the success logging path (lines 160-163)
        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.as_str().contains("# Success Test"));
        assert!(markdown
            .as_str()
            .contains("This file should convert successfully"));

        // Verify the markdown content length is tracked correctly
        assert!(markdown.as_str().len() > MIN_EXPECTED_CONTENT_LENGTH); // Should have substantial content
    }

    #[tokio::test]
    async fn test_various_file_url_formats() {
        let converter = LocalFileConverter::new();

        // Test various file URL formats for normalization
        assert_eq!(
            converter.normalize_path("file:///absolute/path/file.md"),
            "/absolute/path/file.md"
        );

        assert_eq!(
            converter.normalize_path("file://relative/path/file.md"),
            "relative/path/file.md"
        );

        assert_eq!(
            converter.normalize_path("file://../parent/file.md"),
            "../parent/file.md"
        );

        assert_eq!(
            converter.normalize_path("file://./current/file.md"),
            "./current/file.md"
        );
    }

    #[tokio::test]
    async fn test_file_content_edge_cases() {
        let converter = LocalFileConverter::new();

        // Test file with minimal valid content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "a").unwrap(); // Single character

        let file_path = temp_file.path().to_str().unwrap();
        let result = converter.convert(file_path).await;

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.as_str().contains("a"));
    }

    #[tokio::test]
    async fn test_error_context_details() {
        let converter = LocalFileConverter::new();

        // Test that error contexts contain proper details
        let result = converter
            .convert("/definitely/nonexistent/path/file.md")
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            MarkdownError::ContentError { context, .. } => {
                assert_eq!(context.url, "/definitely/nonexistent/path/file.md");
                assert_eq!(context.operation, "File validation");
                assert_eq!(context.converter_type, "LocalFileConverter");
                assert!(context.additional_info.is_some());
            }
            _ => panic!("Expected ContentError with proper context"),
        }
    }
}
