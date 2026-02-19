// sah rule ignore acp/capability-enforcement
//! Content-based search tool using ripgrep library

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{AgentTool, BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use async_trait::async_trait;
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use walkdir::WalkDir;

/// Represents a single grep match
#[derive(Debug, Clone)]
pub struct GrepMatch {
    pub file_path: PathBuf,
    pub line_number: u64,
    pub matched_text: String,
}

/// Results from a grep operation
#[derive(Debug)]
pub struct GrepResults {
    pub matches: Vec<GrepMatch>,
    pub files_searched: usize,
    pub search_time_ms: u64,
}

/// Tool for content-based search using ripgrep library
pub struct GrepFileTool;

impl GrepFileTool {
    pub async fn new() -> Self {
        Self
    }

    fn search_file(
        matcher: &RegexMatcher,
        searcher: &mut Searcher,
        path: &Path,
    ) -> Result<Vec<GrepMatch>, McpError> {
        let matches: Arc<Mutex<Vec<GrepMatch>>> = Arc::new(Mutex::new(Vec::new()));
        let path_buf = path.to_path_buf();
        let matches_clone = Arc::clone(&matches);

        let result = searcher.search_path(
            matcher,
            path,
            UTF8(|line_num, line| {
                let mut m = matches_clone.lock().unwrap();
                m.push(GrepMatch {
                    file_path: path_buf.clone(),
                    line_number: line_num,
                    matched_text: line.trim_end().to_string(),
                });
                Ok(true)
            }),
        );

        match result {
            Ok(_) => {
                let m = matches.lock().unwrap();
                Ok(m.clone())
            }
            Err(_) => Ok(vec![]), // Skip files that can't be searched
        }
    }

    fn matches_file_type(path: &Path, file_type: &str) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match file_type.to_lowercase().as_str() {
                "rust" | "rs" => ext == "rs",
                "python" | "py" => ext == "py",
                "javascript" | "js" => ext == "js",
                "typescript" | "ts" => ext == "ts",
                "json" => ext == "json",
                "yaml" | "yml" => ext == "yaml" || ext == "yml",
                "toml" => ext == "toml",
                "markdown" | "md" => ext == "md",
                "txt" => ext == "txt",
                "html" => ext == "html" || ext == "htm",
                "css" => ext == "css",
                "go" => ext == "go",
                "java" => ext == "java",
                "cpp" | "c++" => ext == "cpp" || ext == "cxx" || ext == "cc",
                "c" => ext == "c" || ext == "h",
                _ => ext == file_type,
            }
        } else {
            false
        }
    }
}

#[derive(serde::Deserialize)]
struct GrepRequest {
    pattern: String,
    #[serde(alias = "file_path", alias = "absolute_path")]
    path: Option<String>,
    glob: Option<String>,
    #[serde(rename = "type")]
    file_type: Option<String>,
    case_insensitive: Option<bool>,
    #[allow(dead_code)]
    context_lines: Option<usize>,
    output_mode: Option<String>,
}

crate::impl_empty_doctorable!(GrepFileTool);

#[async_trait]
impl AgentTool for GrepFileTool {}

#[async_trait]
impl McpTool for GrepFileTool {
    fn name(&self) -> &'static str {
        "files_grep"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        json!({
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
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: GrepRequest = BaseToolImpl::parse_arguments(arguments)?;
        let validator = FilePathValidator::new();

        let search_dir = match &request.path {
            Some(path_str) => {
                let validated_path = validator.validate_path(path_str)?;
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

        let token = generate_progress_token();

        if let Some(sender) = &context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &token,
                    Some(0),
                    format!("Searching for: {}", request.pattern),
                    json!({ "pattern": request.pattern }),
                )
                .ok();
        }

        let start_time = Instant::now();

        // Build the regex matcher
        let matcher = if request.case_insensitive.unwrap_or(false) {
            RegexMatcher::new_line_matcher(&format!("(?i){}", request.pattern))
        } else {
            RegexMatcher::new_line_matcher(&request.pattern)
        }
        .map_err(|e| McpError::invalid_request(format!("Invalid regex pattern: {}", e), None))?;

        // Build the searcher
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0))
            .line_number(true)
            .build();

        let output_mode = request.output_mode.as_deref().unwrap_or("content");
        let mut all_matches: Vec<GrepMatch> = Vec::new();
        let mut files_with_matches = std::collections::HashSet::new();

        // Build glob pattern matcher if specified
        let glob_pattern = request
            .glob
            .as_ref()
            .map(|g| glob::Pattern::new(g))
            .transpose()
            .map_err(|e| McpError::invalid_request(format!("Invalid glob pattern: {}", e), None))?;

        // Walk directory and search files
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
                if !Self::matches_file_type(path, file_type) {
                    continue;
                }
            }

            // Apply glob filter
            if let Some(ref pattern) = glob_pattern {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if !pattern.matches(filename) {
                        continue;
                    }
                }
            }

            let matches = Self::search_file(&matcher, &mut searcher, path)?;
            if !matches.is_empty() {
                files_with_matches.insert(path.to_path_buf());
                all_matches.extend(matches);
            }
        }

        let search_time_ms = start_time.elapsed().as_millis() as u64;

        let results = GrepResults {
            matches: all_matches,
            files_searched: files_with_matches.len(),
            search_time_ms,
        };

        // Format response
        let response = match output_mode {
            "count" => {
                format!(
                    "{} matches in {} files | Time: {}ms",
                    results.matches.len(),
                    results.files_searched,
                    results.search_time_ms
                )
            }
            "files_with_matches" => {
                if files_with_matches.is_empty() {
                    format!(
                        "No files found with matches | Time: {}ms",
                        results.search_time_ms
                    )
                } else {
                    let files: Vec<String> = files_with_matches
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect();
                    format!(
                        "Files with matches ({}):\n{}\n| Time: {}ms",
                        files_with_matches.len(),
                        files.join("\n"),
                        results.search_time_ms
                    )
                }
            }
            _ => {
                // content mode
                if results.matches.is_empty() {
                    format!("No matches found | Time: {}ms", results.search_time_ms)
                } else {
                    let match_lines: Vec<String> = results
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
                        "Found {} matches in {} files | Time: {}ms\n\n{}",
                        results.matches.len(),
                        results.files_searched,
                        results.search_time_ms,
                        match_lines.join("\n")
                    )
                }
            }
        };

        if let Some(sender) = &context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &token,
                    Some(100),
                    format!("Complete: {} matches", results.matches.len()),
                    json!({
                        "total_matches": results.matches.len(),
                        "files_with_matches": results.files_searched,
                        "duration_ms": results.search_time_ms
                    }),
                )
                .ok();
        }

        Ok(BaseToolImpl::create_success_response(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[tokio::test]
    async fn test_grep_file_tool_new() {
        let tool = GrepFileTool::new().await;
        assert_eq!(tool.name(), "files_grep");
    }

    #[tokio::test]
    async fn test_grep_file_tool_schema() {
        let tool = GrepFileTool::new().await;
        let schema = tool.schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
    }

    #[tokio::test]
    async fn test_grep_basic_search() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Hello world\ntest line\nanother test").unwrap();

        let tool = GrepFileTool::new().await;
        let mut arguments = serde_json::Map::new();
        arguments.insert("pattern".to_string(), json!("test"));
        arguments.insert(
            "path".to_string(),
            json!(temp_dir.path().display().to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());
    }
}
