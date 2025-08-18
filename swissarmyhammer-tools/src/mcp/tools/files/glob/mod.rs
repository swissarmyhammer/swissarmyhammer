//! File pattern matching tool for MCP operations
//!
//! This module provides the GlobFileTool for fast file pattern matching with advanced filtering.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use async_trait::async_trait;
use ignore::WalkBuilder;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::path::Path;
use std::time::SystemTime;

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
        _context: &ToolContext,
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
                let validated_path = validator.validate_absolute_path(&path_str)?;
                if !validated_path.exists() {
                    return Err(rmcp::Error::invalid_request(
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
                rmcp::Error::internal_error(format!("Failed to get current directory: {}", e), None)
            })?,
        };

        let respect_git_ignore = request.respect_git_ignore.unwrap_or(true);
        let case_sensitive = request.case_sensitive.unwrap_or(false);

        // Use advanced gitignore integration with ignore crate
        let matched_files = if respect_git_ignore {
            find_files_with_gitignore(&search_dir, &request.pattern, case_sensitive)?
        } else {
            find_files_with_glob(&search_dir, &request.pattern, case_sensitive)?
        };

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
            return Err(rmcp::Error::invalid_request(
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
    let entries = glob::glob_with(&glob_pattern, glob_options)
        .map_err(|e| rmcp::Error::invalid_request(format!("Invalid glob pattern: {}", e), None))?;

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
        return Err(rmcp::Error::invalid_request(
            "Pattern cannot be empty".to_string(),
            None,
        ));
    }

    // Check for extremely long patterns that might cause performance issues
    if pattern.len() > 1000 {
        return Err(rmcp::Error::invalid_request(
            "Pattern is too long (maximum 1000 characters)".to_string(),
            None,
        ));
    }

    // Validate pattern syntax by trying to compile it
    if let Err(e) = glob::Pattern::new(pattern) {
        return Err(rmcp::Error::invalid_request(
            format!("Invalid glob pattern: {}", e),
            None,
        ));
    }

    // Check for potentially problematic patterns
    if pattern.starts_with('/') && !Path::new(pattern).is_absolute() {
        return Err(rmcp::Error::invalid_request(
            "Pattern cannot start with '/' unless it's an absolute path".to_string(),
            None,
        ));
    }

    Ok(())
}
