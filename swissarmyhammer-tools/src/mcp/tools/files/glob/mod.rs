//! File pattern matching tool for MCP operations
//!
//! This module provides the GlobFileTool for fast file pattern matching with advanced filtering.

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use async_trait::async_trait;
use ignore::WalkBuilder;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::path::Path;
use std::time::{Instant, SystemTime};

/// Tool for fast file pattern matching with advanced filtering and sorting
#[derive(Default)]
pub struct GlobFileTool;

impl GlobFileTool {
    /// Creates a new instance of the GlobFileTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GlobFileTool {
    fn name(&self) -> &'static str {
        "files_glob"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., **/*.js, src/**/*.ts)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search within (optional)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case-sensitive matching (default: false)",
                    "default": false
                },
                "respect_git_ignore": {
                    "type": "boolean",
                    "description": "Honor .gitignore patterns (default: true)",
                    "default": true
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct GlobRequest {
            pattern: String,
            path: Option<String>,
            case_sensitive: Option<bool>,
            respect_git_ignore: Option<bool>,
        }

        // Parse arguments
        let request: GlobRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Validate pattern
        validate_glob_pattern(&request.pattern)?;

        // Use FilePathValidator for comprehensive security validation
        let validator = FilePathValidator::new();

        // Determine starting directory
        let search_dir = match request.path {
            Some(path_str) => {
                // Use comprehensive security validation
                let validated_path = validator.validate_path(&path_str)?;
                if !validated_path.exists() {
                    return Err(rmcp::ErrorData::invalid_request(
                        format!(
                            "Search directory does not exist: {}",
                            validated_path.display()
                        ),
                        None,
                    ));
                }
                validated_path
            }
            None => std::env::current_dir().map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("Failed to get current directory: {}", e),
                    None,
                )
            })?,
        };

        let respect_git_ignore = request.respect_git_ignore.unwrap_or(true);
        let case_sensitive = request.case_sensitive.unwrap_or(false);

        // Generate unique token to correlate start and completion notifications for this glob operation.
        // Clients use this token to track progress through the notification lifecycle.
        let token = generate_progress_token();
        let start_time = Instant::now();

        // Send start notification (0% progress) with search parameters.
        // Non-blocking: notification failures don't affect glob operation (uses .ok()).
        // Metadata includes pattern, path, and search options for client context.
        if let Some(sender) = &context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &token,
                    Some(0),
                    format!("Matching pattern: {}", request.pattern),
                    json!({
                        "pattern": request.pattern,
                        "path": search_dir.display().to_string(),
                        "case_sensitive": case_sensitive,
                        "respect_git_ignore": respect_git_ignore
                    }),
                )
                .ok();
        }

        // Use advanced gitignore integration with ignore crate
        let matched_files = if respect_git_ignore {
            find_files_with_gitignore(&search_dir, &request.pattern, case_sensitive)?
        } else {
            find_files_with_glob(&search_dir, &request.pattern, case_sensitive)?
        };

        // Calculate operation duration and file count for completion notification.
        // Duration measured from start_time to provide performance feedback.
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let file_count = matched_files.len();

        // Send completion notification (100% progress) with results summary.
        // Non-blocking: notification failures don't affect tool response (uses .ok()).
        // Metadata includes file count and duration for client feedback and performance monitoring.
        if let Some(sender) = &context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &token,
                    Some(100),
                    format!("Found {} matching files", file_count),
                    json!({
                        "file_count": file_count,
                        "duration_ms": duration_ms
                    }),
                )
                .ok();
        }

        // Format response
        let mut response_parts = Vec::new();

        if !matched_files.is_empty() {
            response_parts.push(format!(
                "Found {} files matching pattern '{}'\n",
                matched_files.len(),
                request.pattern
            ));
            response_parts.push(matched_files.join("\n"));
        } else {
            response_parts.push(format!(
                "No files found matching pattern '{}'",
                request.pattern
            ));
        }

        Ok(BaseToolImpl::create_success_response(
            response_parts.join("\n"),
        ))
    }
}

/// Maximum number of files to return (performance optimization)
const MAX_RESULTS: usize = 10_000;

/// Advanced file search using ignore crate for proper .gitignore support
fn find_files_with_gitignore(
    search_dir: &Path,
    pattern: &str,
    case_sensitive: bool,
) -> Result<Vec<String>, McpError> {
    let mut builder = WalkBuilder::new(search_dir);

    // Configure ignore patterns
    builder
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .parents(true)
        .hidden(false); // Include hidden files but respect ignore patterns

    let walker = builder.build();
    let mut matched_files = Vec::new();

    // Compile glob pattern once for efficiency
    let glob_pattern = match glob::Pattern::new(pattern) {
        Ok(p) => p,
        Err(e) => {
            return Err(rmcp::ErrorData::invalid_request(
                format!("Invalid glob pattern: {}", e),
                None,
            ));
        }
    };

    // Set match options for case sensitivity
    let mut match_options = glob::MatchOptions::new();
    match_options.case_sensitive = case_sensitive;
    match_options.require_literal_separator = false;
    match_options.require_literal_leading_dot = false;

    for entry in walker {
        // Performance optimization: stop if we have enough results
        if matched_files.len() >= MAX_RESULTS {
            tracing::warn!(
                "Result limit reached ({} files), stopping search",
                MAX_RESULTS
            );
            break;
        }

        match entry {
            Ok(dir_entry) => {
                let path = dir_entry.path();

                // Only process files, not directories
                if path.is_file() {
                    let mut matched = false;

                    // For patterns like "*.txt", match against filename
                    if !pattern.contains('/') && !pattern.starts_with("**") {
                        if let Some(file_name) = path.file_name() {
                            if glob_pattern
                                .matches_with(file_name.to_string_lossy().as_ref(), match_options)
                            {
                                matched = true;
                            }
                        }
                    }

                    // For patterns like "**/*.rs" or "src/**/*.py", match against relative path
                    if !matched {
                        if let Ok(relative_path) = path.strip_prefix(search_dir) {
                            if glob_pattern.matches_with(
                                relative_path.to_string_lossy().as_ref(),
                                match_options,
                            ) {
                                matched = true;
                            }
                        }
                    }

                    // For pattern "**/*" (match all files), always match
                    if !matched && (pattern == "**/*" || pattern == "**") {
                        matched = true;
                    }

                    if matched {
                        matched_files.push(path.to_string_lossy().to_string());
                    }
                }
            }
            Err(err) => {
                // Log error but continue processing
                tracing::warn!("Error walking directory: {}", err);
            }
        }
    }

    // Sort by modification time (most recent first)
    sort_files_by_modification_time(&mut matched_files);

    Ok(matched_files)
}

/// Fallback file search using basic glob without gitignore support
fn find_files_with_glob(
    search_dir: &Path,
    pattern: &str,
    case_sensitive: bool,
) -> Result<Vec<String>, McpError> {
    // Build search pattern - if pattern is already absolute, use as-is
    // Otherwise, join with search directory
    let glob_pattern = if Path::new(pattern).is_absolute() {
        pattern.to_string()
    } else {
        search_dir.join(pattern).to_string_lossy().to_string()
    };

    // Configure glob options
    let mut glob_options = glob::MatchOptions::new();
    glob_options.case_sensitive = case_sensitive;
    glob_options.require_literal_separator = false;
    glob_options.require_literal_leading_dot = false;

    // Execute glob pattern
    let entries = glob::glob_with(&glob_pattern, glob_options).map_err(|e| {
        rmcp::ErrorData::invalid_request(format!("Invalid glob pattern: {}", e), None)
    })?;

    let mut matched_files = Vec::new();

    for entry in entries {
        // Performance optimization: stop if we have enough results
        if matched_files.len() >= MAX_RESULTS {
            tracing::warn!(
                "Result limit reached ({} files), stopping search",
                MAX_RESULTS
            );
            break;
        }

        match entry {
            Ok(path) => {
                // Only include files, not directories
                if path.is_file() {
                    matched_files.push(path.to_string_lossy().to_string());
                }
            }
            Err(err) => {
                tracing::warn!("Error accessing file: {}", err);
            }
        }
    }

    // Sort by modification time (most recent first)
    sort_files_by_modification_time(&mut matched_files);

    Ok(matched_files)
}

/// Sort files by modification time (most recent first)
fn sort_files_by_modification_time(files: &mut [String]) {
    files.sort_by(|a, b| {
        let a_metadata = std::fs::metadata(a).ok();
        let b_metadata = std::fs::metadata(b).ok();

        match (a_metadata, b_metadata) {
            (Some(a_meta), Some(b_meta)) => {
                let a_time = a_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let b_time = b_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                b_time.cmp(&a_time) // Most recent first
            }
            (Some(_), None) => std::cmp::Ordering::Less, // Files with metadata come first
            (None, Some(_)) => std::cmp::Ordering::Greater, // Files with metadata come first
            (None, None) => a.cmp(b),                    // Fallback to lexicographic
        }
    });
}

/// Validate glob pattern for common issues
fn validate_glob_pattern(pattern: &str) -> Result<(), McpError> {
    // Check for empty pattern
    if pattern.trim().is_empty() {
        return Err(rmcp::ErrorData::invalid_request(
            "Pattern cannot be empty".to_string(),
            None,
        ));
    }

    // Check for extremely long patterns that might cause performance issues
    if pattern.len() > 1000 {
        return Err(rmcp::ErrorData::invalid_request(
            "Pattern is too long (maximum 1000 characters)".to_string(),
            None,
        ));
    }

    // Validate pattern syntax by trying to compile it
    if let Err(e) = glob::Pattern::new(pattern) {
        return Err(rmcp::ErrorData::invalid_request(
            format!("Invalid glob pattern: {}", e),
            None,
        ));
    }

    // Check for potentially problematic patterns
    if pattern.starts_with('/') && !Path::new(pattern).is_absolute() {
        return Err(rmcp::ErrorData::invalid_request(
            "Pattern cannot start with '/' unless it's an absolute path".to_string(),
            None,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::progress_notifications::ProgressSender;
    use crate::test_utils::create_test_context;
    use tokio::sync::mpsc;

    #[test]
    fn test_glob_file_tool_new() {
        let tool = GlobFileTool::new();
        assert_eq!(tool.name(), "files_glob");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_glob_file_tool_schema() {
        let tool = GlobFileTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["case_sensitive"].is_object());
        assert!(schema["properties"]["respect_git_ignore"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["pattern"]));
    }

    #[tokio::test]
    async fn test_glob_file_tool_sends_progress_notifications() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        // Create a temporary directory with test files
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_dir = temp_dir.path();

        // Create test files
        for i in 0..5 {
            let test_file = test_dir.join(format!("test_{}.rs", i));
            std::fs::write(&test_file, format!("// Test file {}\n", i))
                .expect("Failed to write test file");
        }

        let tool = GlobFileTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("**/*.rs".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(test_dir.display().to_string()),
        );

        // Execute the tool
        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        // Collect all notifications
        let mut notifications = Vec::new();
        while let Ok(notification) = rx.try_recv() {
            notifications.push(notification);
        }

        // Verify we received 2 notifications (start and complete)
        assert_eq!(
            notifications.len(),
            2,
            "Expected 2 notifications, got {}",
            notifications.len()
        );

        // Verify start notification
        let start = &notifications[0];
        assert_eq!(start.progress, Some(0));
        assert!(start.message.contains("Matching pattern"));
        assert!(start.metadata.is_some());
        let start_meta = start.metadata.as_ref().unwrap();
        assert_eq!(start_meta["pattern"], "**/*.rs");
        assert_eq!(start_meta["case_sensitive"], false);
        assert_eq!(start_meta["respect_git_ignore"], true);

        // Verify completion notification
        let complete = &notifications[1];
        assert_eq!(complete.progress, Some(100));
        assert!(complete.message.contains("Found"));
        assert!(complete.message.contains("matching files"));
        assert!(complete.metadata.is_some());
        let complete_meta = complete.metadata.as_ref().unwrap();
        assert!(complete_meta["file_count"].is_number());
        assert!(complete_meta["duration_ms"].is_number());
        // Verify we found 5 test files
        assert_eq!(complete_meta["file_count"], 5);
    }

    #[tokio::test]
    async fn test_glob_file_tool_works_without_progress_sender() {
        let context = create_test_context().await;

        // Create a temporary directory with test files
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_dir = temp_dir.path();

        // Create test file
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, "test content\n").expect("Failed to write test file");

        let tool = GlobFileTool::new();
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("*.txt".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(test_dir.display().to_string()),
        );

        // Execute the tool - should succeed even without progress sender
        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_glob_validates_pattern() {
        let context = create_test_context().await;
        let tool = GlobFileTool::new();

        // Test empty pattern
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }
}
