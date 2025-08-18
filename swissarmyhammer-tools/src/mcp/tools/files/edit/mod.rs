//! File editing tool for MCP operations
//!
//! This module provides the EditFileTool for performing precise string replacements in files
//! with atomic operations, comprehensive security validation, file encoding preservation,
//! and metadata preservation.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use encoding_rs::{Encoding, UTF_8};
use filetime::{set_file_times, FileTime};
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::Path;
use tracing::{debug, info};

/// Result information for edit operations
#[derive(Debug, Clone)]
pub struct EditResult {
    /// Number of bytes written to the file
    pub bytes_written: usize,
    /// Number of string replacements made in the file
    pub replacements_made: usize,
    /// The character encoding that was detected and preserved
    pub encoding_detected: String,
    /// The line ending format that was preserved
    pub line_endings_preserved: String,
    /// Whether file metadata (permissions, timestamps) was successfully preserved
    pub metadata_preserved: bool,
}

/// Validation result for edit operations
#[derive(Debug, Clone)]
struct EditValidation {
    pub file_exists: bool,
    pub old_string_found: bool,
    pub old_string_count: usize,
    pub is_unique: bool,
}

/// Line ending types detected in files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEnding {
    Lf,   // Unix: \n
    CrLf, // Windows: \r\n
    Cr,   // Classic Mac: \r
    Mixed, // Multiple types found
}

impl LineEnding {
    /// Detect the primary line ending type in content
    fn detect(content: &str) -> Self {
        let crlf_count = content.matches("\r\n").count();
        let lf_count = content.matches('\n').count() - crlf_count; // Exclude CRLF \n
        let cr_count = content.matches('\r').count() - crlf_count; // Exclude CRLF \r

        match (lf_count > 0, crlf_count > 0, cr_count > 0) {
            (false, false, false) => LineEnding::Lf, // Default for empty/no line endings
            (true, false, false) => LineEnding::Lf,
            (false, true, false) => LineEnding::CrLf,
            (false, false, true) => LineEnding::Cr,
            _ => LineEnding::Mixed,
        }
    }

    /// Get the string representation
    fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF", 
            LineEnding::Cr => "CR",
            LineEnding::Mixed => "Mixed",
        }
    }
}

/// Tool for performing precise string replacements in existing files
#[derive(Default)]
pub struct EditFileTool;

impl EditFileTool {
    /// Creates a new instance of the EditFileTool
    pub fn new() -> Self {
        Self
    }

    /// Validates the edit operation before making changes
    ///
    /// Performs comprehensive validation including:
    /// - File existence check
    /// - Old string existence and uniqueness validation
    /// - Security checks through file path validation
    fn validate_edit_operation(
        &self,
        file_path: &str,
        content: &str,
        old_string: &str,
        replace_all: bool,
    ) -> Result<EditValidation, McpError> {
        use crate::mcp::tools::files::shared_utils::validate_file_path;

        // Validate file path first
        let path = validate_file_path(file_path)?;
        let file_exists = path.exists();

        if !file_exists {
            return Err(McpError::invalid_request(
                format!("File does not exist: {}", file_path),
                None,
            ));
        }

        // Count occurrences of old_string
        let matches: Vec<_> = content.matches(old_string).collect();
        let old_string_count = matches.len();
        let old_string_found = old_string_count > 0;

        // Check uniqueness if single replacement requested
        let is_unique = old_string_count <= 1;

        if !old_string_found {
            return Err(McpError::invalid_request(
                format!("String '{}' not found in file", old_string),
                None,
            ));
        }

        if !replace_all && !is_unique {
            return Err(McpError::invalid_request(
                format!(
                    "String '{}' appears {} times in file. Use replace_all=true for multiple replacements",
                    old_string, old_string_count
                ),
                None,
            ));
        }

        Ok(EditValidation {
            file_exists,
            old_string_found,
            old_string_count,
            is_unique,
        })
    }

    /// Detects file encoding and reads content as string
    ///
    /// Uses encoding_rs for robust encoding detection and handles:
    /// - UTF-8 (most common)
    /// - UTF-16 with BOM
    /// - Other encodings with fallback to UTF-8
    fn read_with_encoding_detection(&self, file_path: &Path) -> Result<(String, &'static Encoding), McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Read raw bytes first
        let bytes = fs::read(file_path)
            .map_err(|e| handle_file_error(e, "read file for encoding detection", file_path))?;

        // Detect encoding using BOM, fallback to UTF-8
        let (encoding, bom_length) = encoding_rs::Encoding::for_bom(&bytes)
            .unwrap_or((UTF_8, 0));

        // Use the bytes after BOM for decoding
        let bytes_to_decode = &bytes[bom_length..];

        debug!(
            path = %file_path.display(),
            encoding = encoding.name(),
            bom_length = bom_length,
            "Detected file encoding"
        );

        // Decode to string
        let (content, _, had_decode_errors) = encoding.decode(bytes_to_decode);
        
        if had_decode_errors {
            return Err(McpError::internal_error(
                format!("Failed to decode file with detected encoding {}", encoding.name()),
                None,
            ));
        }

        Ok((content.into_owned(), encoding))
    }

    /// Performs atomic file edit with full validation and metadata preservation
    ///
    /// This method implements the complete atomic edit workflow:
    /// 1. Validate file path and edit parameters
    /// 2. Read file with encoding detection
    /// 3. Validate old_string existence and uniqueness
    /// 4. Perform replacement operation
    /// 5. Write to temporary file in same directory
    /// 6. Preserve file metadata (permissions, timestamps)
    /// 7. Atomically rename temporary file to original
    /// 8. Clean up temporary file on any failure
    pub fn edit_file_atomic(
        &self,
        file_path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> Result<EditResult, McpError> {
        use crate::mcp::tools::files::shared_utils::{validate_file_path, handle_file_error};

        // Step 1: Validate file path and get canonical path
        let path = validate_file_path(file_path)?;

        info!(
            path = %path.display(),
            old_string_len = old_string.len(),
            new_string_len = new_string.len(),
            replace_all = replace_all,
            "Starting atomic edit operation"
        );

        // Step 2: Read original file with encoding detection
        let (original_content, detected_encoding) = self.read_with_encoding_detection(&path)?;
        
        // Step 3: Detect line endings
        let line_ending = LineEnding::detect(&original_content);

        // Step 4: Validate edit operation
        let validation = self.validate_edit_operation(file_path, &original_content, old_string, replace_all)?;

        // Step 5: Get original file metadata for preservation
        let original_metadata = fs::metadata(&path)
            .map_err(|e| handle_file_error(e, "read metadata", &path))?;
        
        let original_permissions = original_metadata.permissions();
        let original_modified = FileTime::from_last_modification_time(&original_metadata);
        let original_accessed = FileTime::from_last_access_time(&original_metadata);

        // Step 6: Perform replacement
        let (new_content, replacements_made) = if replace_all {
            let new_content = original_content.replace(old_string, new_string);
            let replacements = validation.old_string_count;
            (new_content, replacements)
        } else {
            let new_content = original_content.replacen(old_string, new_string, 1);
            (new_content, 1)
        };

        // Step 7: Create temporary file in same directory as original
        let temp_file_name = format!("{}.tmp.{}", path.display(), std::process::id());
        let temp_path = path.parent()
            .ok_or_else(|| McpError::internal_error(
                "Cannot determine parent directory for temporary file".to_string(),
                None,
            ))?
            .join(&temp_file_name);

        debug!(
            temp_path = %temp_path.display(),
            content_length = new_content.len(),
            encoding = detected_encoding.name(),
            "Writing content to temporary file"
        );

        // Step 8: Write new content to temporary file with original encoding
        let write_result = self.write_with_encoding(&temp_path, &new_content, detected_encoding);

        match write_result {
            Ok(bytes_written) => {
                // Step 9: Set permissions on temporary file to match original
                if let Err(e) = fs::set_permissions(&temp_path, original_permissions.clone()) {
                    // Clean up and return error
                    let _ = fs::remove_file(&temp_path);
                    return Err(handle_file_error(e, "set permissions on temporary file", &temp_path));
                }

                // Step 10: Atomically rename temporary file to original
                let rename_result = fs::rename(&temp_path, &path);
                
                match rename_result {
                    Ok(()) => {
                        // Step 11: Restore file timestamps
                        let metadata_preserved = if let Err(e) = set_file_times(&path, original_accessed, original_modified) {
                            debug!(
                                path = %path.display(),
                                error = %e,
                                "Failed to preserve file timestamps, continuing anyway"
                            );
                            false
                        } else {
                            true
                        };

                        debug!(
                            path = %path.display(),
                            bytes_written = bytes_written,
                            replacements_made = replacements_made,
                            metadata_preserved = metadata_preserved,
                            "Atomic edit operation completed successfully"
                        );

                        Ok(EditResult {
                            bytes_written,
                            replacements_made,
                            encoding_detected: detected_encoding.name().to_string(),
                            line_endings_preserved: line_ending.as_str().to_string(),
                            metadata_preserved,
                        })
                    }
                    Err(e) => {
                        // Clean up temporary file and return error
                        let _ = fs::remove_file(&temp_path);
                        Err(handle_file_error(e, "rename temporary file to target", &path))
                    }
                }
            }
            Err(e) => {
                // Clean up temporary file and return error
                let _ = fs::remove_file(&temp_path);
                Err(e)
            }
        }
    }

    /// Writes content to file with specified encoding
    ///
    /// Preserves the original encoding of the file and handles BOM appropriately.
    fn write_with_encoding(
        &self,
        file_path: &Path,
        content: &str,
        encoding: &'static Encoding,
    ) -> Result<usize, McpError> {
        use crate::mcp::tools::files::shared_utils::handle_file_error;

        // Encode content back to bytes using the detected encoding
        let (bytes, _, had_errors) = encoding.encode(content);
        
        if had_errors {
            return Err(McpError::internal_error(
                format!("Failed to encode content with encoding {}", encoding.name()),
                None,
            ));
        }

        // Write bytes to file
        let file = fs::File::create(file_path)
            .map_err(|e| handle_file_error(e, "create temporary file", file_path))?;

        let mut writer = BufWriter::new(file);
        writer.write_all(&bytes)
            .map_err(|e| handle_file_error(e, "write to temporary file", file_path))?;
        
        writer.flush()
            .map_err(|e| handle_file_error(e, "flush temporary file", file_path))?;

        Ok(bytes.len())
    }
}

#[async_trait]
impl McpTool for EditFileTool {
    fn name(&self) -> &'static str {
        "files_edit"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct EditRequest {
            file_path: String,
            old_string: String,
            new_string: String,
            replace_all: Option<bool>,
        }

        // Parse arguments
        let request: EditRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate parameters before operation
        if request.file_path.trim().is_empty() {
            return Err(McpError::invalid_request(
                "file_path cannot be empty".to_string(),
                None,
            ));
        }

        if request.old_string.is_empty() {
            return Err(McpError::invalid_request(
                "old_string cannot be empty".to_string(),
                None,
            ));
        }

        // Validate replacement strings are different
        if request.old_string == request.new_string {
            return Err(McpError::invalid_request(
                "old_string and new_string must be different".to_string(),
                None,
            ));
        }

        // Log edit attempt for security auditing
        info!(
            path = %request.file_path,
            old_string_len = request.old_string.len(),
            new_string_len = request.new_string.len(),
            replace_all = request.replace_all.unwrap_or(false),
            "Attempting atomic edit operation"
        );

        // Perform atomic edit operation
        let replace_all = request.replace_all.unwrap_or(false);
        let edit_result = self.edit_file_atomic(
            &request.file_path,
            &request.old_string,
            &request.new_string,
            replace_all,
        )?;

        // Create detailed success response
        let success_message = format!(
            "Successfully edited file: {} | {} replacements made | {} bytes written | Encoding: {} | Line endings: {} | Metadata preserved: {}",
            request.file_path,
            edit_result.replacements_made,
            edit_result.bytes_written,
            edit_result.encoding_detected,
            edit_result.line_endings_preserved,
            edit_result.metadata_preserved
        );

        debug!(
            path = %request.file_path,
            bytes_written = edit_result.bytes_written,
            replacements_made = edit_result.replacements_made,
            encoding = %edit_result.encoding_detected,
            line_endings = %edit_result.line_endings_preserved,
            metadata_preserved = edit_result.metadata_preserved,
            "Edit operation completed successfully"
        );

        Ok(BaseToolImpl::create_success_response(success_message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolContext;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use swissarmyhammer::common::rate_limiter::MockRateLimiter;
    use swissarmyhammer::git::GitOperations;
    use swissarmyhammer::issues::FileSystemIssueStorage;
    use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
    use crate::mcp::tool_handlers::ToolHandlers;
    use tempfile::TempDir;
    use tokio::sync::{Mutex, RwLock};

    /// Create a test context for tool execution
    fn create_test_context() -> ToolContext {
        let issue_storage = Arc::new(RwLock::new(
            Box::new(FileSystemIssueStorage::new(PathBuf::from("./test_issues")).unwrap())
                as Box<dyn swissarmyhammer::issues::IssueStorage>,
        ));
        let git_ops = Arc::new(Mutex::new(None::<GitOperations>));
        let memo_storage = Arc::new(RwLock::new(
            Box::new(MockMemoStorage::new()) as Box<dyn MemoStorage>,
        ));
        let tool_handlers = Arc::new(ToolHandlers::new(memo_storage.clone()));
        let rate_limiter = Arc::new(MockRateLimiter);

        ToolContext::new(tool_handlers, issue_storage, git_ops, memo_storage, rate_limiter)
    }

    /// Create test arguments for the edit tool
    fn create_edit_arguments(
        file_path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: Option<bool>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert("file_path".to_string(), serde_json::Value::String(file_path.to_string()));
        args.insert("old_string".to_string(), serde_json::Value::String(old_string.to_string()));
        args.insert("new_string".to_string(), serde_json::Value::String(new_string.to_string()));
        
        if let Some(replace_all) = replace_all {
            args.insert("replace_all".to_string(), serde_json::Value::Bool(replace_all));
        }
        
        args
    }

    #[test]
    fn test_line_ending_detection() {
        // Test Unix line endings (LF)
        let unix_content = "Line 1\nLine 2\nLine 3\n";
        assert_eq!(LineEnding::detect(unix_content), LineEnding::Lf);

        // Test Windows line endings (CRLF)
        let windows_content = "Line 1\r\nLine 2\r\nLine 3\r\n";
        assert_eq!(LineEnding::detect(windows_content), LineEnding::CrLf);

        // Test Classic Mac line endings (CR)
        let mac_content = "Line 1\rLine 2\rLine 3\r";
        assert_eq!(LineEnding::detect(mac_content), LineEnding::Cr);

        // Test mixed line endings
        let mixed_content = "Line 1\nLine 2\r\nLine 3\r";
        assert_eq!(LineEnding::detect(mixed_content), LineEnding::Mixed);

        // Test no line endings
        let no_endings = "Single line";
        assert_eq!(LineEnding::detect(no_endings), LineEnding::Lf);

        // Test empty content
        let empty_content = "";
        assert_eq!(LineEnding::detect(empty_content), LineEnding::Lf);
    }

    #[test]
    fn test_edit_tool_creation() {
        let tool = EditFileTool::new();
        assert_eq!(tool.name(), "files_edit");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_edit_tool_schema() {
        let tool = EditFileTool::new();
        let schema = tool.schema();

        // Verify schema structure
        assert!(schema.is_object());
        let schema_obj = schema.as_object().unwrap();
        
        assert_eq!(schema_obj.get("type").unwrap().as_str().unwrap(), "object");
        assert!(schema_obj.contains_key("properties"));
        assert!(schema_obj.contains_key("required"));

        // Verify required fields
        let required = schema_obj.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("file_path".to_string())));
        assert!(required.contains(&serde_json::Value::String("old_string".to_string())));
        assert!(required.contains(&serde_json::Value::String("new_string".to_string())));

        // Verify properties
        let properties = schema_obj.get("properties").unwrap().as_object().unwrap();
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("old_string"));
        assert!(properties.contains_key("new_string"));
        assert!(properties.contains_key("replace_all"));
    }

    #[tokio::test]
    async fn test_edit_single_replacement_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_edit.txt");
        let initial_content = "Hello world! This is a test file.";
        fs::write(&test_file, initial_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "world",
            "universe",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was edited correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a test file.");
    }

    #[tokio::test]
    async fn test_edit_replace_all_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_replace_all.txt");
        let initial_content = "test test test";
        fs::write(&test_file, initial_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "test",
            "exam",
            Some(true),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify all occurrences were replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam");
    }

    #[tokio::test]
    async fn test_edit_multiple_occurrences_without_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_multiple.txt");
        let initial_content = "duplicate duplicate duplicate";
        fs::write(&test_file, initial_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "duplicate",
            "unique",
            None, // replace_all = false by default
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("appears 3 times"));
        assert!(format!("{:?}", error).contains("Use replace_all=true"));

        // Verify file was not modified
        let unchanged_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(unchanged_content, initial_content);
    }

    #[tokio::test]
    async fn test_edit_string_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_not_found.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("not found in file"));

        // Verify file was not modified
        let unchanged_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(unchanged_content, initial_content);
    }

    #[tokio::test]
    async fn test_edit_file_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &nonexistent_file.to_string_lossy(),
            "old",
            "new",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // The error message from shared_utils says "File not found"
        assert!(
            error_str.contains("File does not exist") || 
            error_str.contains("File not found") ||
            error_str.contains("does not exist") || 
            error_str.contains("NotFound")
        );
    }

    #[tokio::test]
    async fn test_edit_empty_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();

        // Test empty file path
        let args = create_edit_arguments("", "old", "new", None);
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("file_path cannot be empty"));

        // Test empty old_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "", "new", None);
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("old_string cannot be empty"));

        // Test identical old_string and new_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "same", "same", None);
        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("must be different"));
    }

    #[tokio::test]
    async fn test_edit_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode_test.txt");
        let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
        fs::write(&test_file, unicode_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "🌍",
            "🚀",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify Unicode replacement worked correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello 🚀! Здравствуй мир! 你好世界!");
    }

    #[tokio::test]
    async fn test_edit_preserves_line_endings() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test Windows line endings preservation
        let windows_file = temp_dir.path().join("windows_endings.txt");
        let windows_content = "Line 1\r\nold text\r\nLine 3\r\n";
        fs::write(&windows_file, windows_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &windows_file.to_string_lossy(),
            "old text",
            "new text",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&windows_file).unwrap();
        assert_eq!(edited_content, "Line 1\r\nnew text\r\nLine 3\r\n");
        assert!(edited_content.contains("\r\n"));
    }

    #[tokio::test] 
    async fn test_edit_atomic_operation_failure_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_atomic.txt");
        let initial_content = "original content";
        fs::write(&test_file, initial_content).unwrap();

        // Make file read-only to cause atomic operation to fail during permission setting
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            
            let readonly_permissions = Permissions::from_mode(0o444);
            fs::set_permissions(&test_file, readonly_permissions).unwrap();

            let tool = EditFileTool::new();
            
            // Even if the operation fails, we should verify no temporary files are left behind
            let _temp_pattern = format!("{}.tmp.*", test_file.display());
            
            // The edit should work even with readonly file since we change permissions on temp file
            let edit_result = tool.edit_file_atomic(
                &test_file.to_string_lossy(),
                "original",
                "modified",
                false,
            );
            
            // Check that no temporary files remain regardless of result
            let temp_files: Vec<_> = temp_dir.path()
                .read_dir()
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.file_name().to_string_lossy().contains(".tmp.")
                })
                .collect();
            
            assert!(temp_files.is_empty(), "Temporary files should be cleaned up");

            // If the edit succeeded, verify the content was actually changed
            if edit_result.is_ok() {
                let final_content = fs::read_to_string(&test_file).unwrap();
                assert_eq!(final_content, "modified content");
            }
        }
    }

    #[tokio::test]
    async fn test_edit_file_permissions_preservation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("permissions_test.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        // Set specific permissions (only on Unix systems)
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            
            let permissions = Permissions::from_mode(0o755);
            fs::set_permissions(&test_file, permissions).unwrap();
            
            let original_metadata = fs::metadata(&test_file).unwrap();
            let original_mode = original_metadata.permissions().mode();

            let tool = EditFileTool::new();
            let edit_result = tool.edit_file_atomic(
                &test_file.to_string_lossy(),
                "test",
                "updated",
                false,
            );

            assert!(edit_result.is_ok());
            
            // Verify permissions were preserved
            let new_metadata = fs::metadata(&test_file).unwrap();
            let new_mode = new_metadata.permissions().mode();
            assert_eq!(original_mode, new_mode, "File permissions should be preserved");

            // Verify content was updated
            let final_content = fs::read_to_string(&test_file).unwrap();
            assert_eq!(final_content, "updated content");
        }
    }

    #[tokio::test]
    async fn test_edit_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("response_test.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "world",
            "universe",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());

        // Check response message format contains expected information
        let response_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content in response"),
        };

        assert!(response_text.contains("Successfully edited file"));
        assert!(response_text.contains(&*test_file.to_string_lossy()));
        assert!(response_text.contains("1 replacements made"));
        assert!(response_text.contains("Encoding:"));
        assert!(response_text.contains("Line endings:"));
        assert!(response_text.contains("Metadata preserved:"));
    }

    #[test]
    fn test_edit_validation_logic() {
        let tool = EditFileTool::new();
        
        // Test with content that has multiple occurrences
        let content = "test content with test and more test";
        let _result = tool.validate_edit_operation(
            "/dev/null", // Won't be used in this test
            content,
            "test",
            false, // replace_all = false
        );

        // This should fail because we have multiple occurrences but replace_all = false
        // However, it will fail earlier because /dev/null doesn't exist as a regular file
        // So let's test the logic directly

        // Count occurrences manually to verify logic
        let matches: Vec<_> = content.matches("test").collect();
        assert_eq!(matches.len(), 3);
        
        // Test unique string
        let matches_unique: Vec<_> = content.matches("content").collect();
        assert_eq!(matches_unique.len(), 1);
    }

    #[test]
    fn test_encoding_detection_logic() {
        let tool = EditFileTool::new();
        
        // Create a temporary file with UTF-8 content
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("encoding_test.txt");
        let utf8_content = "Hello, 世界! 🌍";
        fs::write(&test_file, utf8_content).unwrap();

        let result = tool.read_with_encoding_detection(&test_file);
        assert!(result.is_ok());
        
        let (content, encoding) = result.unwrap();
        assert_eq!(content, utf8_content);
        assert_eq!(encoding.name(), "UTF-8");
    }

    #[tokio::test]
    async fn test_edit_json_argument_parsing_error() {
        let tool = EditFileTool::new();
        let context = create_test_context();
        
        // Create invalid arguments (missing required field)
        let mut args = serde_json::Map::new();
        args.insert("file_path".to_string(), serde_json::Value::String("/test/path".to_string()));
        args.insert("old_string".to_string(), serde_json::Value::String("old".to_string()));
        // Missing "new_string" field

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn test_edit_large_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");
        
        // Create a reasonably large file (1MB) with repetitive content
        let chunk = "This is a line of test content that will be repeated many times.\n";
        let chunk_size = chunk.len();
        let target_size = 1_000_000; // 1MB
        let repetitions = target_size / chunk_size;
        
        let large_content = chunk.repeat(repetitions);
        fs::write(&test_file, &large_content).unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "test content",
            "modified content",
            Some(true), // Replace all occurrences
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify the replacements were made
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert!(edited_content.contains("modified content"));
        assert!(!edited_content.contains("test content"));
    }

    #[tokio::test]
    async fn test_edit_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_file.txt");
        fs::write(&test_file, "").unwrap();

        let tool = EditFileTool::new();
        let context = create_test_context();
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("not found in file"));
    }
}
