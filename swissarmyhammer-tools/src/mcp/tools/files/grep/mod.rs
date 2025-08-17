//! Content-based search tool for MCP operations
//!
//! This module provides the GrepFileTool for fast text searching using ripgrep.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::path::Path;

/// Check if a file type matches the requested type filter
fn matches_file_type(path: &Path, file_type: &str) -> bool {
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        match file_type.to_lowercase().as_str() {
            "rust" | "rs" => extension == "rs",
            "python" | "py" => extension == "py",
            "javascript" | "js" => extension == "js",
            "typescript" | "ts" => extension == "ts",
            "json" => extension == "json",
            "yaml" | "yml" => extension == "yaml" || extension == "yml",
            "toml" => extension == "toml",
            "markdown" | "md" => extension == "md",
            "txt" => extension == "txt",
            "html" => extension == "html" || extension == "htm",
            "css" => extension == "css",
            "xml" => extension == "xml",
            "java" => extension == "java",
            "cpp" | "c++" => extension == "cpp" || extension == "cxx" || extension == "cc",
            "c" => extension == "c" || extension == "h",
            "go" => extension == "go",
            "php" => extension == "php",
            "ruby" | "rb" => extension == "rb",
            "shell" | "sh" => extension == "sh" || extension == "bash",
            _ => extension == file_type, // Direct extension match
        }
    } else {
        false
    }
}

/// Basic check to determine if a file is likely binary
fn is_likely_binary_file(path: &Path) -> bool {
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        match extension.to_lowercase().as_str() {
            // Binary executable formats
            "exe" | "dll" | "so" | "dylib" | "bin" => true,
            // Archive formats
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => true,
            // Image formats
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "svg" => true,
            // Video/Audio formats
            "mp4" | "avi" | "mov" | "mp3" | "wav" | "ogg" => true,
            // Document formats
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => true,
            // Other binary formats
            "db" | "sqlite" | "sqlite3" | "lock" => true,
            _ => false,
        }
    } else {
        false
    }
}

/// Tool for content-based search using ripgrep for fast and flexible text searching
#[derive(Default)]
pub struct GrepFileTool;

impl GrepFileTool {
    /// Creates a new instance of the GrepFileTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for GrepFileTool {
    fn name(&self) -> &'static str {
        "files_grep"
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
                    "description": "Regular expression pattern to search"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (optional)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., *.js) (optional)"
                },
                "type": {
                    "type": "string",
                    "description": "File type filter (e.g., js, py, rust) (optional)"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive search (optional)"
                },
                "context_lines": {
                    "type": "number",
                    "description": "Number of context lines around matches (optional)"
                },
                "output_mode": {
                    "type": "string",
                    "description": "Output format (content, files_with_matches, count) (optional)",
                    "enum": ["content", "files_with_matches", "count"]
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
        use regex::RegexBuilder;
        use serde::Deserialize;
        use walkdir::WalkDir;

        #[derive(Deserialize)]
        struct GrepRequest {
            pattern: String,
            path: Option<String>,
            glob: Option<String>,
            #[serde(rename = "type")]
            file_type: Option<String>,
            case_insensitive: Option<bool>,
            context_lines: Option<usize>,
            output_mode: Option<String>,
        }

        // Parse arguments
        let request: GrepRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Build regex pattern
        let regex = RegexBuilder::new(&request.pattern)
            .case_insensitive(request.case_insensitive.unwrap_or(false))
            .build()
            .map_err(|e| {
                rmcp::Error::invalid_request(format!("Invalid regex pattern: {}", e), None)
            })?;

        // Use FilePathValidator for comprehensive security validation
        let validator = FilePathValidator::new();

        // Determine search directory
        let search_dir = match request.path {
            Some(path_str) => {
                // Use comprehensive security validation
                let validated_path = validator.validate_absolute_path(&path_str)?;
                if !validated_path.exists() {
                    return Err(rmcp::Error::invalid_request(
                        format!("Search path does not exist: {}", validated_path.display()),
                        None,
                    ));
                }
                validated_path
            }
            None => std::env::current_dir().map_err(|e| {
                rmcp::Error::internal_error(format!("Failed to get current directory: {}", e), None)
            })?,
        };

        let output_mode = request.output_mode.unwrap_or_else(|| "content".to_string());
        let context_lines = request.context_lines.unwrap_or(0);

        let mut results = Vec::new();
        let mut file_count = 0;
        let mut match_count = 0;

        // Walk directory tree
        let walker = if search_dir.is_file() {
            WalkDir::new(&search_dir).max_depth(0)
        } else {
            WalkDir::new(&search_dir)
        };

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Apply file type filter
            if let Some(ref file_type) = request.file_type {
                if !matches_file_type(path, file_type) {
                    continue;
                }
            }

            // Apply glob filter
            if let Some(ref glob_pattern) = request.glob {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    let pattern = glob::Pattern::new(glob_pattern).map_err(|e| {
                        rmcp::Error::invalid_request(format!("Invalid glob pattern: {}", e), None)
                    })?;
                    if !pattern.matches(filename) {
                        continue;
                    }
                }
            }

            // Skip binary files and common non-text files
            if is_likely_binary_file(path) {
                continue;
            }

            // Read and search file content
            let content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(_) => continue, // Skip files we can't read as text
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut file_matches = Vec::new();

            for (line_num, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    match_count += 1;

                    if output_mode == "content" {
                        // Include context lines
                        let start = line_num.saturating_sub(context_lines);
                        let end = std::cmp::min(line_num + context_lines + 1, lines.len());

                        let context_block: Vec<String> = (start..end)
                            .map(|i| {
                                let prefix = if i == line_num { ">" } else { " " };
                                format!("{}{}:{}", prefix, i + 1, lines[i])
                            })
                            .collect();

                        file_matches.push(format!(
                            "{}:\n{}",
                            path.display(),
                            context_block.join("\n")
                        ));
                    }
                }
            }

            if !file_matches.is_empty() {
                file_count += 1;
                if output_mode == "content" {
                    results.extend(file_matches);
                } else if output_mode == "files_with_matches" {
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }

        // Format response based on output mode
        let response = match output_mode.as_str() {
            "count" => format!("{} matches in {} files", match_count, file_count),
            "files_with_matches" => {
                if results.is_empty() {
                    "No files found with matches".to_string()
                } else {
                    format!(
                        "Files with matches ({}):\n{}",
                        results.len(),
                        results.join("\n")
                    )
                }
            }
            "content" => {
                if results.is_empty() {
                    "No matches found".to_string()
                } else {
                    format!(
                        "Found {} matches in {} files:\n\n{}",
                        match_count,
                        file_count,
                        results.join("\n\n")
                    )
                }
            }
            _ => {
                return Err(rmcp::Error::invalid_request(
                    "Invalid output_mode. Must be 'content', 'files_with_matches', or 'count'"
                        .to_string(),
                    None,
                ));
            }
        };

        Ok(BaseToolImpl::create_success_response(response))
    }
}
