//! File pattern matching tool for MCP operations
//!
//! This module provides the GlobFileTool for fast file pattern matching with advanced filtering.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;

/// Basic check for common ignore patterns
/// In production, you might want to use the ignore crate for full .gitignore support
fn should_ignore_path(path: &str) -> bool {
    // Common patterns to ignore
    let ignore_patterns = [
        ".git/", ".gitignore", ".DS_Store", "node_modules/", "target/",
        ".vscode/", ".idea/", "*.tmp", "*.swp", "*.swo", "*.bak", "Thumbs.db",
    ];
    
    for pattern in &ignore_patterns {
        if pattern.ends_with('/') {
            // Directory pattern
            if path.contains(&pattern[..pattern.len()-1]) {
                return true;
            }
        } else if pattern.contains('*') {
            // Wildcard pattern (basic support)
            let pattern_base = pattern.trim_start_matches('*').trim_end_matches('*');
            if path.contains(pattern_base) {
                return true;
            }
        } else {
            // Exact match
            if path.ends_with(pattern) || path.contains(&format!("/{}", pattern)) {
                return true;
            }
        }
    }
    
    false
}

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
        use std::path::Path;

        #[derive(Deserialize)]
        struct GlobRequest {
            pattern: String,
            path: Option<String>,
            case_sensitive: Option<bool>,
            respect_git_ignore: Option<bool>,
        }

        // Parse arguments
        let request: GlobRequest = BaseToolImpl::parse_arguments(arguments)?;
        
        // Determine starting directory
        let search_dir = match request.path {
            Some(path_str) => {
                let path_buf = std::path::PathBuf::from(&path_str);
                if !path_buf.is_absolute() {
                    return Err(rmcp::Error::invalid_request(
                        "Search path must be absolute".to_string(),
                        None,
                    ));
                }
                if !path_buf.exists() {
                    return Err(rmcp::Error::invalid_request(
                        format!("Search directory does not exist: {}", path_buf.display()),
                        None,
                    ));
                }
                path_buf
            }
            None => std::env::current_dir().map_err(|e| {
                rmcp::Error::internal_error(format!("Failed to get current directory: {}", e), None)
            })?,
        };

        // Build search pattern - if pattern is already absolute, use as-is
        // Otherwise, join with search directory
        let glob_pattern = if Path::new(&request.pattern).is_absolute() {
            request.pattern.clone()
        } else {
            search_dir.join(&request.pattern).to_string_lossy().to_string()
        };

        // Configure glob options
        let mut glob_options = glob::MatchOptions::new();
        glob_options.case_sensitive = request.case_sensitive.unwrap_or(false);
        glob_options.require_literal_separator = false;
        glob_options.require_literal_leading_dot = false;

        // Execute glob pattern
        let entries = glob::glob_with(&glob_pattern, glob_options)
            .map_err(|e| {
                rmcp::Error::invalid_request(format!("Invalid glob pattern: {}", e), None)
            })?;

        let mut matched_files: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for entry in entries {
            match entry {
                Ok(path) => {
                    // Apply git ignore filtering if requested
                    let respect_git_ignore = request.respect_git_ignore.unwrap_or(true);
                    if respect_git_ignore {
                        // Basic .gitignore check - in a real implementation you might want to use
                        // the ignore crate for more sophisticated ignore patterns
                        let path_str = path.to_string_lossy();
                        if should_ignore_path(&path_str) {
                            continue;
                        }
                    }
                    
                    matched_files.push(path.to_string_lossy().to_string());
                }
                Err(e) => {
                    errors.push(format!("Error accessing file: {}", e));
                }
            }
        }

        // Sort results by modification time (most recent first)
        matched_files.sort_by(|a, b| {
            let a_metadata = std::fs::metadata(a).ok();
            let b_metadata = std::fs::metadata(b).ok();
            
            match (a_metadata, b_metadata) {
                (Some(a_meta), Some(b_meta)) => {
                    b_meta.modified().unwrap_or(std::time::UNIX_EPOCH)
                        .cmp(&a_meta.modified().unwrap_or(std::time::UNIX_EPOCH))
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.cmp(b), // Fallback to lexicographic
            }
        });

        // Format response
        let mut response_parts = Vec::new();
        
        if !matched_files.is_empty() {
            response_parts.push(format!("Found {} files matching pattern '{}'\n", matched_files.len(), request.pattern));
            response_parts.push(matched_files.join("\n"));
        } else {
            response_parts.push(format!("No files found matching pattern '{}'", request.pattern));
        }

        if !errors.is_empty() {
            response_parts.push(format!("\nErrors encountered:\n{}", errors.join("\n")));
        }

        Ok(BaseToolImpl::create_success_response(response_parts.join("\n")))
    }
}