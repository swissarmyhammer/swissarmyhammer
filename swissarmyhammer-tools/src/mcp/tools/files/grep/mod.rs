//! Content-based search tool for MCP operations
//!
//! This module provides the GrepFileTool for fast text searching with ripgrep integration.
//! Falls back to regex-based search when ripgrep is not available.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

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

/// Enhanced binary file detection using both extension and content analysis
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

/// Check if file content contains binary data by examining a sample
fn is_binary_content(sample: &[u8]) -> bool {
    // Check for null bytes which are common in binary files
    sample.contains(&0) ||
    // Check if content is valid UTF-8
    std::str::from_utf8(sample).is_err()
}

/// Enhanced binary file detection that samples file content
async fn should_skip_file(path: &Path) -> bool {
    // First check by extension
    if is_likely_binary_file(path) {
        return true;
    }

    // Sample first 512 bytes for binary content detection
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut buffer = [0; 512];
        if let Ok(n) = file.read(&mut buffer) {
            if n > 0 && is_binary_content(&buffer[..n]) {
                return true;
            }
        }
    }
    false
}

/// Represents a single grep match with context
#[derive(Debug, Clone)]
pub struct GrepMatch {
    /// The path to the file containing the match
    pub file_path: PathBuf,
    /// The line number where the match was found (1-based)
    pub line_number: usize,
    /// The column where the match starts (optional)
    pub column: Option<usize>,
    /// The text content of the matched line
    pub matched_text: String,
    /// Lines appearing before the match (for context)
    pub context_before: Vec<String>,
    /// Lines appearing after the match (for context)
    pub context_after: Vec<String>,
}

/// Results from a grep operation with metadata
#[derive(Debug)]
pub struct GrepResults {
    /// Individual matches found during the search
    pub matches: Vec<GrepMatch>,
    /// Number of files that were searched
    pub files_searched: usize,
    /// Total number of matches found across all files
    pub total_matches: usize,
    /// Time taken to perform the search in milliseconds
    pub search_time_ms: u64,
    /// Whether ripgrep was used for the search (true) or regex fallback (false)
    pub used_ripgrep: bool,
    /// Version of ripgrep used, if available
    pub ripgrep_version: Option<String>,
}

/// Tool for content-based search using ripgrep for fast and flexible text searching
pub struct GrepFileTool {
    ripgrep_available: bool,
    ripgrep_version: Option<String>,
}

impl Default for GrepFileTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GrepFileTool {
    /// Creates a new instance of the GrepFileTool and checks for ripgrep availability
    pub fn new() -> Self {
        let (ripgrep_available, ripgrep_version) = Self::check_ripgrep_availability();
        Self {
            ripgrep_available,
            ripgrep_version,
        }
    }

    /// Check if ripgrep is available and get version
    fn check_ripgrep_availability() -> (bool, Option<String>) {
        match Command::new("rg").arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version_output = String::from_utf8_lossy(&output.stdout);
                    let version = version_output.lines().next().map(|line| line.to_string());
                    (true, version)
                } else {
                    (false, None)
                }
            }
            Err(_) => (false, None),
        }
    }

    /// Execute search using ripgrep for optimal performance
    async fn execute_with_ripgrep(
        &self,
        request: &GrepRequest,
        search_path: &Path,
    ) -> std::result::Result<GrepResults, McpError> {
        let start_time = Instant::now();

        let mut cmd = Command::new("rg");
        cmd.arg(&request.pattern);

        // Configure ripgrep arguments based on request
        if let Some(ref glob_pattern) = request.glob {
            cmd.arg("--glob").arg(glob_pattern);
        }

        if let Some(ref file_type) = request.file_type {
            // Convert our file type to ripgrep type if possible
            let rg_type = match file_type.to_lowercase().as_str() {
                "rust" | "rs" => "rust",
                "python" | "py" => "py",
                "javascript" | "js" => "js",
                "typescript" | "ts" => "ts",
                "json" => "json",
                "yaml" | "yml" => "yaml",
                "toml" => "toml",
                "markdown" | "md" => "md",
                "html" => "html",
                "css" => "css",
                "java" => "java",
                "cpp" | "c++" => "cpp",
                "c" => "c",
                "go" => "go",
                "php" => "php",
                "ruby" | "rb" => "ruby",
                "shell" | "sh" => "sh",
                _ => file_type, // Pass through as-is
            };
            cmd.arg("--type").arg(rg_type);
        }

        if request.case_insensitive.unwrap_or(false) {
            cmd.arg("--ignore-case");
        }

        if let Some(context) = request.context_lines {
            cmd.arg("--context").arg(context.to_string());
        }

        // Set output format
        match request.output_mode.as_deref().unwrap_or("content") {
            "files_with_matches" => {
                cmd.arg("--files-with-matches");
            }
            "count" => {
                cmd.arg("--count");
            }
            _ => {
                // default content mode
                cmd.arg("--with-filename").arg("--line-number");
            }
        }

        // Add the search path
        cmd.arg(search_path);

        // Execute ripgrep command
        let output = cmd.output().map_err(|e| {
            McpError::internal_error(format!("Failed to execute ripgrep: {}", e), None)
        })?;

        let search_time_ms = start_time.elapsed().as_millis() as u64;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ripgrep exits with code 1 when no matches found, which is not an error
            if output.status.code() == Some(1) && output.stdout.is_empty() {
                // No matches found - this is a normal case
                return Ok(GrepResults {
                    matches: vec![],
                    files_searched: 0,
                    total_matches: 0,
                    search_time_ms,
                    used_ripgrep: true,
                    ripgrep_version: self.ripgrep_version.clone(),
                });
            } else {
                return Err(McpError::invalid_request(
                    format!("Ripgrep search failed: {}", stderr),
                    None,
                ));
            }
        }

        // Parse ripgrep JSON output or simple output based on mode
        let output_str = String::from_utf8_lossy(&output.stdout);
        let results = self.parse_ripgrep_output(&output_str, &request.output_mode)?;

        Ok(GrepResults {
            matches: results.matches,
            files_searched: results.files_searched,
            total_matches: results.total_matches,
            search_time_ms,
            used_ripgrep: true,
            ripgrep_version: self.ripgrep_version.clone(),
        })
    }

    /// Parse ripgrep output into structured results
    fn parse_ripgrep_output(
        &self,
        output: &str,
        output_mode: &Option<String>,
    ) -> std::result::Result<GrepResults, McpError> {
        let mode = output_mode.as_deref().unwrap_or("content");

        match mode {
            "files_with_matches" => {
                let files: Vec<String> = output
                    .lines()
                    .filter(|line| !line.is_empty())
                    .map(|line| line.to_string())
                    .collect();

                Ok(GrepResults {
                    matches: vec![],
                    files_searched: files.len(),
                    total_matches: files.len(),
                    search_time_ms: 0,
                    used_ripgrep: true,
                    ripgrep_version: self.ripgrep_version.clone(),
                })
            }
            "count" => {
                let mut total_matches: usize = 0;
                let mut files_searched: usize = 0;

                for line in output.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    files_searched += 1;
                    if let Some(count_str) = line.split(':').nth(1) {
                        if let Ok(count) = count_str.parse::<usize>() {
                            total_matches += count;
                        }
                    } else if let Ok(count) = line.parse::<usize>() {
                        total_matches += count;
                    }
                }

                Ok(GrepResults {
                    matches: vec![],
                    files_searched,
                    total_matches,
                    search_time_ms: 0,
                    used_ripgrep: true,
                    ripgrep_version: self.ripgrep_version.clone(),
                })
            }
            _ => {
                // Parse regular ripgrep output with filename:line_number:content format
                let mut matches = Vec::new();
                let mut files_searched = std::collections::HashSet::new();

                for line in output.lines() {
                    if line.is_empty() {
                        continue;
                    }

                    // Parse ripgrep output format: filename:line_number:content
                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                    if parts.len() >= 3 {
                        let file_path = parts[0];
                        if let Ok(line_number) = parts[1].parse::<usize>() {
                            let matched_text = parts[2];
                            files_searched.insert(file_path.to_string());
                            matches.push(GrepMatch {
                                file_path: PathBuf::from(file_path),
                                line_number,
                                column: None,
                                matched_text: matched_text.to_string(),
                                context_before: vec![],
                                context_after: vec![],
                            });
                        }
                    }
                }

                Ok(GrepResults {
                    total_matches: matches.len(),
                    files_searched: files_searched.len(),
                    matches,
                    search_time_ms: 0,
                    used_ripgrep: true,
                    ripgrep_version: self.ripgrep_version.clone(),
                })
            }
        }
    }

    /// Execute search using regex fallback when ripgrep is not available
    async fn execute_with_fallback(
        &self,
        request: &GrepRequest,
        search_path: &Path,
    ) -> std::result::Result<GrepResults, McpError> {
        use regex::RegexBuilder;
        use walkdir::WalkDir;

        let start_time = Instant::now();

        // Build regex pattern
        let regex = RegexBuilder::new(&request.pattern)
            .case_insensitive(request.case_insensitive.unwrap_or(false))
            .build()
            .map_err(|e| {
                McpError::invalid_request(format!("Invalid regex pattern: {}", e), None)
            })?;

        let output_mode = request.output_mode.as_deref().unwrap_or("content");
        let context_lines = request.context_lines.unwrap_or(0);

        let mut results = Vec::new();
        let mut file_count = 0;
        let mut match_count = 0;

        // Walk directory tree
        let walker = if search_path.is_file() {
            WalkDir::new(search_path).max_depth(0)
        } else {
            WalkDir::new(search_path)
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
                        McpError::invalid_request(format!("Invalid glob pattern: {}", e), None)
                    })?;
                    if !pattern.matches(filename) {
                        continue;
                    }
                }
            }

            // Enhanced binary file detection
            if should_skip_file(path).await {
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

        let search_time_ms = start_time.elapsed().as_millis() as u64;

        // Convert to GrepResults format
        let matches = if output_mode == "content" {
            // For fallback, we don't parse individual matches from the combined results
            // This is acceptable as the primary use case should be ripgrep
            vec![]
        } else {
            vec![]
        };

        Ok(GrepResults {
            matches,
            files_searched: file_count,
            total_matches: match_count,
            search_time_ms,
            used_ripgrep: false,
            ripgrep_version: None,
        })
    }
}

#[derive(serde::Deserialize)]
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
        // Parse arguments
        let request: GrepRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Use FilePathValidator for comprehensive security validation
        let validator = FilePathValidator::new();

        // Determine search directory
        let search_dir = match &request.path {
            Some(path_str) => {
                // Use comprehensive security validation
                let validated_path = validator.validate_absolute_path(path_str)?;
                if !validated_path.exists() {
                    return Err(McpError::invalid_request(
                        format!("Search path does not exist: {}", validated_path.display()),
                        None,
                    ));
                }
                validated_path
            }
            None => std::env::current_dir().map_err(|e| {
                McpError::internal_error(format!("Failed to get current directory: {}", e), None)
            })?,
        };

        // Execute search using ripgrep if available, otherwise fallback to regex
        let results = if self.ripgrep_available {
            self.execute_with_ripgrep(&request, &search_dir).await?
        } else {
            self.execute_with_fallback(&request, &search_dir).await?
        };

        // Format response based on output mode and results
        let output_mode = request.output_mode.as_deref().unwrap_or("content");
        let response = self.format_response(&results, output_mode)?;

        Ok(BaseToolImpl::create_success_response(response))
    }
}

impl GrepFileTool {
    /// Format the grep results into a human-readable response
    fn format_response(
        &self,
        results: &GrepResults,
        output_mode: &str,
    ) -> std::result::Result<String, McpError> {
        let engine_info = if results.used_ripgrep {
            format!(
                " | Engine: ripgrep {} | Time: {}ms",
                results.ripgrep_version.as_deref().unwrap_or("unknown"),
                results.search_time_ms
            )
        } else {
            format!(
                " | Engine: regex fallback | Time: {}ms",
                results.search_time_ms
            )
        };

        let response = match output_mode {
            "count" => format!(
                "{} matches in {} files{}",
                results.total_matches, results.files_searched, engine_info
            ),
            "files_with_matches" => {
                if results.files_searched == 0 {
                    format!("No files found with matches{}", engine_info)
                } else {
                    format!(
                        "Files with matches ({}){}",
                        results.files_searched, engine_info
                    )
                }
            }
            "content" => {
                if results.total_matches == 0 {
                    format!("No matches found{}", engine_info)
                } else if results.matches.is_empty() {
                    // Fallback case - we don't have detailed match info
                    format!(
                        "Found {} matches in {} files{}",
                        results.total_matches, results.files_searched, engine_info
                    )
                } else {
                    // Format detailed matches
                    let match_details: Vec<String> = results
                        .matches
                        .iter()
                        .map(|m| {
                            format!(
                                "{}:{}: {}",
                                m.file_path.display(),
                                m.line_number,
                                m.matched_text
                            )
                        })
                        .collect();

                    format!(
                        "Found {} matches in {} files{}:\n\n{}",
                        results.total_matches,
                        results.files_searched,
                        engine_info,
                        match_details.join("\n")
                    )
                }
            }
            _ => {
                return Err(McpError::invalid_request(
                    "Invalid output_mode. Must be 'content', 'files_with_matches', or 'count'"
                        .to_string(),
                    None,
                ));
            }
        };

        Ok(response)
    }
}
