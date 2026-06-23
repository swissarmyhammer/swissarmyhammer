// sah rule ignore acp/capability-enforcement
//! Content-based search tool using ripgrep library

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, ToolContext};
use crate::mcp::tools::files::shared_utils::{reject_filesystem_root, FilePathValidator};
use grep::regex::RegexMatcher;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::WalkBuilder;
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use std::path::{Path, PathBuf};
use std::time::Instant;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Default number of context lines shown around each match in `content` mode
/// when the caller does not specify `context_lines`.
///
/// Plain ripgrep defaults to zero context, but a bare matching line is rarely
/// enough for an agent to act on, so `grep files` leans toward ripgrep's common
/// `-C2` invocation: two lines before and after. Callers wanting the terse
/// one-line-per-match output pass `context_lines: 0` explicitly.
pub const DEFAULT_CONTEXT_LINES: usize = 2;

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
        .description("Lines of context before and after each match in 'content' mode (optional; defaults to 2, like ripgrep -C2; pass 0 for one line per match)")
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

/// Represents a single emitted line — either a matching line or a surrounding
/// context line (when `context_lines > 0`).
#[derive(Debug, Clone)]
pub struct GrepMatch {
    pub file_path: PathBuf,
    pub line_number: u64,
    pub matched_text: String,
    /// `true` for a line that matched the pattern, `false` for a context line
    /// emitted around a match.
    pub is_match: bool,
}

/// Sink that collects both matching lines and the surrounding context lines the
/// searcher emits when before/after context is configured. The stock
/// `sinks::UTF8` sink only forwards matches (and errors when context is on), so
/// supporting `context_lines` requires implementing [`Sink`] directly.
struct CollectSink<'a> {
    path: &'a Path,
    out: &'a mut Vec<GrepMatch>,
}

/// Decode a line emitted by the searcher into trimmed UTF-8, matching the
/// previous `line.trim_end()` behavior.
fn decode_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_end().to_string()
}

impl Sink for CollectSink<'_> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, std::io::Error> {
        let start = mat.line_number().unwrap_or(0);
        // A single SinkMatch can span multiple lines; number them sequentially
        // from the match's starting line.
        for (offset, line) in mat.lines().enumerate() {
            self.out.push(GrepMatch {
                file_path: self.path.to_path_buf(),
                line_number: start + offset as u64,
                matched_text: decode_line(line),
                is_match: true,
            });
        }
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, std::io::Error> {
        self.out.push(GrepMatch {
            file_path: self.path.to_path_buf(),
            line_number: ctx.line_number().unwrap_or(0),
            matched_text: decode_line(ctx.bytes()),
            is_match: false,
        });
        Ok(true)
    }
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
        let mut out: Vec<GrepMatch> = Vec::new();
        let result = searcher.search_path(
            matcher,
            path,
            CollectSink {
                path,
                out: &mut out,
            },
        );

        match result {
            Ok(_) => Ok(out),
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
    #[serde(
        default,
        deserialize_with = "crate::mcp::tools::files::shared_utils::deserialize_flexible_usize"
    )]
    context_lines: Option<usize>,
    output_mode: Option<String>,
}

/// Execute a grep content search operation
pub async fn execute_grep(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: GrepRequest = BaseToolImpl::parse_arguments(arguments)?;

    // The session working directory (the board dir) is the root for an unscoped
    // search and the base for resolving a relative `path`. Never the process CWD,
    // which is `/` for the bundled GUI app.
    let session_root = context.session_root();
    let validator = FilePathValidator::new(session_root.clone());

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
        None => session_root,
    };

    // Defensive guard: never walk the entire filesystem or the process CWD. A
    // search rooted at `/` (the original "grep hung forever" failure) or at an
    // unresolved relative `.` would visit far too much; refuse both.
    reject_filesystem_root(&search_dir)?;

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

    let output_mode = request.output_mode.as_deref().unwrap_or("content");

    // Context lines only make sense for `content` output; `count` and
    // `files_with_matches` summarize and never print surrounding lines, so we
    // skip the extra work there. Default to ripgrep-style -C2 when unspecified.
    let context_lines = if output_mode == "content" {
        request.context_lines.unwrap_or(DEFAULT_CONTEXT_LINES)
    } else {
        0
    };

    // Build the searcher
    let mut searcher_builder = SearcherBuilder::new();
    searcher_builder
        .binary_detection(BinaryDetection::quit(0))
        .line_number(true);
    if context_lines > 0 {
        searcher_builder
            .before_context(context_lines)
            .after_context(context_lines);
    }
    let mut searcher = searcher_builder.build();

    let mut all_matches: Vec<GrepMatch> = Vec::new();
    let mut files_with_matches = std::collections::HashSet::new();

    // Build glob pattern matcher if specified
    let glob_pattern = request
        .glob
        .as_ref()
        .map(|g| glob::Pattern::new(g))
        .transpose()
        .map_err(|e| McpError::invalid_request(format!("Invalid glob pattern: {}", e), None))?;

    // Walk directory and search files.
    //
    // Use `ignore::WalkBuilder` (ripgrep's own walker) so the search honors
    // `.gitignore`/`.ignore`, skips the `.git` directory, and — crucially —
    // does not descend into ignored build output like `target/`. A raw
    // `walkdir::WalkDir` rooted high in the tree walks every file unconditionally,
    // which is what let an unscoped grep run forever.
    let mut builder = WalkBuilder::new(&search_dir);
    builder
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .parents(true)
        .hidden(true); // skip hidden files/dirs (e.g. `.git`) like ripgrep does
    if search_dir.is_file() {
        builder.max_depth(Some(0));
    }
    let walker = builder.build();

    for entry in walker.filter_map(|e| e.ok()) {
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
        // `matches` may include context-only lines; a file counts as a hit only
        // when it has at least one actual matching line.
        if matches.iter().any(|m| m.is_match) {
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

    // Only matching lines count toward totals; context lines are presentation.
    let match_count = results.matches.iter().filter(|m| m.is_match).count();

    // Format response
    let response = match output_mode {
        "count" => {
            format!(
                "{} matches in {} files | Time: {}ms",
                match_count, results.files_searched, results.search_time_ms
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
            if match_count == 0 {
                format!("No matches found | Time: {}ms", results.search_time_ms)
            } else {
                // ripgrep-style rendering: matching lines use a `:` separator,
                // context lines use `-`, and a `--` divider marks a break
                // between non-adjacent hunks (different file or a line gap).
                // The divider is only meaningful when context is shown.
                let mut out_lines: Vec<String> = Vec::new();
                let mut prev: Option<(&Path, u64)> = None;
                for m in &results.matches {
                    if context_lines > 0 {
                        if let Some((prev_path, prev_line)) = prev {
                            if prev_path != m.file_path.as_path() || m.line_number > prev_line + 1 {
                                out_lines.push("--".to_string());
                            }
                        }
                    }
                    let sep = if m.is_match { ':' } else { '-' };
                    out_lines.push(format!(
                        "{}{}{}{} {}",
                        m.file_path.display(),
                        sep,
                        m.line_number,
                        sep,
                        m.matched_text
                    ));
                    prev = Some((m.file_path.as_path(), m.line_number));
                }
                format!(
                    "Found {} matches in {} files | Time: {}ms\n\n{}",
                    match_count,
                    results.files_searched,
                    results.search_time_ms,
                    out_lines.join("\n")
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
            match_count, results.search_time_ms
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

    /// Extract the text payload from a grep `CallToolResult`.
    fn result_text(result: Result<CallToolResult, McpError>) -> String {
        let call_result = result.expect("grep should succeed");
        match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        }
    }

    async fn grep_content(
        context: &ToolContext,
        file: &Path,
        args: &[(&str, serde_json::Value)],
    ) -> String {
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("charlie".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(file.display().to_string()),
        );
        for (k, v) in args {
            arguments.insert(k.to_string(), v.clone());
        }
        result_text(execute_grep(arguments, context).await)
    }

    /// With no `context_lines`, content mode defaults to ripgrep-style -C2:
    /// the match prints with a `:` separator and surrounding lines with `-`.
    #[tokio::test]
    async fn test_grep_default_context_lines() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file = temp_dir.path().join("ctx.txt");
        std::fs::write(&file, "alpha\nbravo\ncharlie\ndelta\necho\n").unwrap();

        let text = grep_content(&context, &file, &[]).await;

        // The match line uses `:`; context lines use `-`.
        assert!(
            text.contains(":3: charlie"),
            "match line with ':' separator: {text}"
        );
        assert!(text.contains("-1- alpha"), "context before: {text}");
        assert!(text.contains("-2- bravo"), "context before: {text}");
        assert!(text.contains("-4- delta"), "context after: {text}");
        assert!(text.contains("-5- echo"), "context after: {text}");
        // Only one actual match.
        assert!(
            text.contains("Found 1 matches"),
            "match count excludes context: {text}"
        );
    }

    /// `context_lines: 0` restores terse one-line-per-match output: only the
    /// matching line, no surrounding lines and no `--` hunk dividers.
    #[tokio::test]
    async fn test_grep_context_lines_zero() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file = temp_dir.path().join("ctx.txt");
        std::fs::write(&file, "alpha\nbravo\ncharlie\ndelta\necho\n").unwrap();

        let text = grep_content(&context, &file, &[("context_lines", serde_json::json!(0))]).await;

        assert!(text.contains(":3: charlie"), "match present: {text}");
        assert!(!text.contains("bravo"), "no context line: {text}");
        assert!(!text.contains("delta"), "no context line: {text}");
        assert!(
            !text.contains("--"),
            "no hunk divider without context: {text}"
        );
    }

    /// An explicit `context_lines` is honored exactly.
    #[tokio::test]
    async fn test_grep_context_lines_explicit() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file = temp_dir.path().join("ctx.txt");
        std::fs::write(&file, "alpha\nbravo\ncharlie\ndelta\necho\n").unwrap();

        let text = grep_content(&context, &file, &[("context_lines", serde_json::json!(1))]).await;

        assert!(text.contains("-2- bravo"), "one line before: {text}");
        assert!(text.contains(":3: charlie"), "match: {text}");
        assert!(text.contains("-4- delta"), "one line after: {text}");
        // Lines outside the 1-line window are not shown.
        assert!(!text.contains("alpha"), "line 1 outside window: {text}");
        assert!(!text.contains("echo"), "line 5 outside window: {text}");
    }

    /// Language models stringify numeric arguments, sending `context_lines` as
    /// `"1"` instead of `1`. The string form must be coerced and behave
    /// identically to the integer form — this pins the `deserialize_with`
    /// wiring on `GrepRequest.context_lines`, not just the shared helper.
    #[tokio::test]
    async fn test_grep_string_context_lines() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file = temp_dir.path().join("ctx.txt");
        std::fs::write(&file, "alpha\nbravo\ncharlie\ndelta\necho\n").unwrap();

        let text = grep_content(
            &context,
            &file,
            &[("context_lines", serde_json::json!("1"))],
        )
        .await;

        // Identical to the integer-input twin (test_grep_context_lines_explicit).
        assert!(text.contains("-2- bravo"), "one line before: {text}");
        assert!(text.contains(":3: charlie"), "match: {text}");
        assert!(text.contains("-4- delta"), "one line after: {text}");
        assert!(!text.contains("alpha"), "line 1 outside window: {text}");
        assert!(!text.contains("echo"), "line 5 outside window: {text}");
    }

    /// Two matches separated by a gap produce a `--` divider between hunks.
    #[tokio::test]
    async fn test_grep_context_hunk_divider() {
        let context = create_test_context().await;
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file = temp_dir.path().join("gap.txt");
        std::fs::write(&file, "charlie\nx\nx\nx\nx\nx\ncharlie\n").unwrap();

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "pattern".to_string(),
            serde_json::Value::String("charlie".to_string()),
        );
        arguments.insert(
            "path".to_string(),
            serde_json::Value::String(file.display().to_string()),
        );
        arguments.insert("context_lines".to_string(), serde_json::json!(1));
        let text = result_text(execute_grep(arguments, &context).await);

        assert!(text.contains("Found 2 matches"), "two matches: {text}");
        assert!(
            text.contains("\n--\n"),
            "hunk divider between non-adjacent matches: {text}"
        );
    }
}
