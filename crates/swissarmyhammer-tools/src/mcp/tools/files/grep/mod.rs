// sah rule ignore acp/capability-enforcement
//! Content-based search tool using ripgrep library

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, ToolContext};
use crate::mcp::tools::files::shared_utils::FilePathValidator;
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder};
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use walkdir::WalkDir;

/// Operation metadata for grep content search
#[derive(Debug, Default)]
pub struct GrepFiles;

static GREP_FILES_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("pattern")
        .description("Regular expression pattern to search")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("path")
        .description("File or directory to search in (optional)")
        .param_type(ParamType::String),
    ParamMeta::new("glob")
        .description("Glob pattern to filter files (e.g., *.js) (optional)")
        .param_type(ParamType::String),
    ParamMeta::new("type")
        .description("File type filter (e.g., js, py, rust) (optional)")
        .param_type(ParamType::String),
    ParamMeta::new("case_insensitive")
        .description("Case-insensitive search (optional)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("context_lines")
        .description("Number of context lines around matches (optional)")
        .param_type(ParamType::Integer),
    ParamMeta::new("output_mode")
        .description("Output format: content, files_with_matches, or count (optional)")
        .param_type(ParamType::String),
];

impl Operation for GrepFiles {
    fn verb(&self) -> &'static str {
        "grep"
    }
    fn noun(&self) -> &'static str {
        "files"
    }
    fn description(&self) -> &'static str {
        "Content-based search using ripgrep for fast text searching"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        GREP_FILES_PARAMS
    }
}

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

/// Execute a grep content search operation
pub async fn execute_grep(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
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

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "grep",
        format!("Searching: {}", request.pattern),
    )
    .await;

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
            if !GrepFileTool::matches_file_type(path, file_type) {
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

        let matches = GrepFileTool::search_file(&matcher, &mut searcher, path)?;
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

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "grep",
        format!(
            "Complete: {} matches in {}ms",
            results.matches.len(),
            results.search_time_ms
        ),
    )
    .await;

    Ok(BaseToolImpl::create_success_response(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_grep_file_tool_metadata() {
        let op = GrepFiles;
        assert_eq!(op.verb(), "grep");
        assert_eq!(op.noun(), "files");
        assert!(!op.description().is_empty());
        assert!(!op.parameters().is_empty());
    }

    #[tokio::test]
    async fn test_grep_basic_search() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "Hello world\ntest line\nanother test").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("test".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_grep_invalid_regex_error() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            // Invalid regex: unmatched bracket
            serde_json::Value::String("[invalid".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("Invalid regex") || err.contains("regex") || err.contains("pattern"));
    }

    #[tokio::test]
    async fn test_grep_output_mode_count() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("count_test.txt");
        std::fs::write(&test_file, "foo\nfoo\nbar\nfoo\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("foo".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );
        arguments.insert(
            "output_mode".to_string(),
            serde_json::Value::String("count".to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // count mode shows "N matches in M files"
        assert!(text.contains("matches"));
        assert!(text.contains("files"));
    }

    #[tokio::test]
    async fn test_grep_output_mode_files_with_matches() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("match.txt");
        let no_match_file = temp_dir.path().join("no_match.txt");
        std::fs::write(&test_file, "hello world\n").unwrap();
        std::fs::write(&no_match_file, "no match here\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("hello".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );
        arguments.insert(
            "output_mode".to_string(),
            serde_json::Value::String("files_with_matches".to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        assert!(text.contains("match.txt") || text.contains("Files with matches"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("case_test.txt");
        std::fs::write(&test_file, "Hello World\nHELLO WORLD\nhello world\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("hello".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );
        arguments.insert(
            "case_insensitive".to_string(),
            serde_json::Value::Bool(true),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // Should find 3 matches (Hello, HELLO, hello)
        assert!(text.contains("3 matches") || text.contains("Found 3"));
    }

    #[tokio::test]
    async fn test_grep_file_type_filter() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let rs_file = temp_dir.path().join("code.rs");
        let txt_file = temp_dir.path().join("notes.txt");
        std::fs::write(&rs_file, "fn hello() {}\n").unwrap();
        std::fs::write(&txt_file, "fn hello world\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("hello".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );
        arguments.insert(
            "type".to_string(),
            serde_json::Value::String("rust".to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // Should only find in .rs file
        assert!(text.contains("code.rs") || text.contains("1 match"));
        assert!(!text.contains("notes.txt"));
    }

    #[tokio::test]
    async fn test_grep_glob_filter() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let rs_file = temp_dir.path().join("code.rs");
        let txt_file = temp_dir.path().join("notes.txt");
        std::fs::write(&rs_file, "hello rust\n").unwrap();
        std::fs::write(&txt_file, "hello text\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("hello".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );
        arguments.insert(
            "glob".to_string(),
            serde_json::Value::String("*.rs".to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // Should only search .rs files
        assert!(text.contains("code.rs") || text.contains("1 match"));
        assert!(!text.contains("notes.txt"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "nothing relevant here\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("xyz_not_found_pattern".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        assert!(text.contains("No matches found") || text.contains("0 matches"));
    }

    #[tokio::test]
    async fn test_grep_search_single_file() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("single.txt");
        std::fs::write(&test_file, "line 1\ntarget line\nline 3\n").unwrap();

        // Point path directly at a file
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("target".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.display().to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        assert!(text.contains("target") || text.contains("1 match"));
    }

    #[tokio::test]
    async fn test_grep_nonexistent_path_error() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("does_not_exist");

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("hello".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(nonexistent.display().to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(
            err.contains("does not exist") || err.contains("not found") || err.contains("NotFound")
        );
    }

    #[tokio::test]
    async fn test_grep_files_with_no_matches_mode() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "no match here\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("xyz_not_present".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(temp_dir.path().display().to_string()),
        );
        arguments.insert(
            "output_mode".to_string(),
            serde_json::Value::String("files_with_matches".to_string()),
        );

        let result = execute_grep(arguments, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        assert!(text.contains("No files found") || text.contains("0"));
    }
}
