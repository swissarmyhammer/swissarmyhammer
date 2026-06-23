// sah rule ignore acp/capability-enforcement
//! File reading handler for MCP operations.
//!
//! This module provides [`execute_read`] — the read-file handler shared between
//! the unified [`crate::mcp::tools::files::FilesTool`] (dispatched via
//! `op: "read file"`) and the validator-facing
//! [`crate::mcp::tools::files::read_file::ReadFileTool`] (called by name).
//! It supports reading UTF-8 text files and partial reads for large files via
//! line-based offset/limit. Non-UTF-8 (binary) content is rejected with an
//! error rather than decoded.
//!
//! Note: This is an MCP tool, not an ACP operation. ACP capability checking happens at the
//! agent layer (claude-agent, llama-agent), not at the MCP tool layer.
//!
//! ## Features
//!
//! * **Comprehensive Security**: All file paths undergo security validation through the enhanced
//!   security framework, including workspace boundary enforcement and path traversal protection
//! * **Partial Reading**: Efficient reading of large files using line-based offset and limit
//!   parameters without loading the entire file into memory
//! * **Text Only**: Reads UTF-8 text; non-UTF-8 (binary) files are rejected with an error
//! * **Performance Optimized**: Configurable limits prevent excessive resource usage
//! * **Audit Logging**: All file access attempts are logged for security monitoring
//!
//! ## Security Considerations
//!
//! All file operations are subject to comprehensive security validation:
//! - Both absolute and relative path support with secure resolution
//! - Workspace boundary enforcement to prevent access outside authorized directories
//! - Path traversal attack prevention (blocking `../` sequences)
//! - Permission checking before file access attempts
//! - Structured audit logging for security monitoring
//!
//! ## Examples
//!
//! ```rust,ignore
//! # use swissarmyhammer_tools::mcp::tool_registry::ToolContext;
//! # use serde_json::json;
//! # async fn example(context: &ToolContext) -> Result<(), rmcp::ErrorData> {
//! use swissarmyhammer_tools::mcp::tools::files::read::execute_read;
//!
//! // Read entire file
//! let mut args = serde_json::Map::new();
//! args.insert("path".to_string(), json!("/workspace/src/main.rs"));
//! let result = execute_read(args, context).await?;
//!
//! // Read with offset and limit
//! let mut args = serde_json::Map::new();
//! args.insert("path".to_string(), json!("/workspace/logs/app.log"));
//! args.insert("offset".to_string(), json!(100));
//! args.insert("limit".to_string(), json!(50));
//! let result = execute_read(args, context).await?;
//! # Ok(())
//! # }
//! ```

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use tracing::{debug, info};

/// Operation metadata for reading files
#[derive(Debug, Default)]
pub struct ReadFile;

static READ_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("path")
        .description("Path to the file to read (absolute or relative to current working directory)")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("offset")
        .description("Starting line number for partial reading (optional)")
        .param_type(ParamType::Integer),
    ParamMeta::new("limit")
        .description("Maximum number of lines to read (optional)")
        .param_type(ParamType::Integer),
    ParamMeta::new("format")
        .description(
            "Output form: \"hashline\" (default) prefixes each text line with a \
             `N:HH|` anchor (absolute 1-based line number + content hash) so the \
             line can be referenced by `edit files`; \"plain\" emits untagged \
             content. Only UTF-8 text is read; non-UTF-8 (binary) files are \
             rejected with an error and are never tagged.",
        )
        .param_type(ParamType::String)
        .allowed_values(&["hashline", "plain"]),
];

/// Output form for [`execute_read`]: hashline-tagged (default) or plain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFormat {
    /// Each text line is prefixed with a `N:HH|` hashline anchor.
    Hashline,
    /// Pre-existing untagged content.
    Plain,
}

impl ReadFormat {
    /// Parse the `format` argument, defaulting to [`ReadFormat::Hashline`].
    ///
    /// `allowed_values` on the param already constrains the schema; this rejects
    /// any other value defensively with an `invalid_request` error.
    fn parse(value: Option<&str>) -> Result<Self, McpError> {
        match value {
            None | Some("hashline") => Ok(ReadFormat::Hashline),
            Some("plain") => Ok(ReadFormat::Plain),
            Some(other) => Err(McpError::invalid_request(
                format!("format must be \"hashline\" or \"plain\", got {other:?}"),
                None,
            )),
        }
    }
}

/// Prefix marker for the whole-file freshness-token metadata line.
///
/// The first line of a successful read is `#hash:<hex>` — the
/// [`whole_file_hash`](crate::mcp::tools::files::shared_utils::whole_file_hash)
/// of the full on-disk bytes. `write files` / `edit files` use it as a
/// staleness token. It precedes the (optionally tagged) content.
const HASH_LINE_PREFIX: &str = "#hash:";

impl Operation for ReadFile {
    fn verb(&self) -> &'static str {
        "read"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Read file contents from the local filesystem"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        READ_FILE_PARAMS
    }
}

/// Execute a file read operation
///
/// This is the shared handler that backs both the unified
/// [`crate::mcp::tools::files::FilesTool`] (dispatched via `op: "read file"`)
/// and the validator-facing
/// [`crate::mcp::tools::files::read_file::ReadFileTool`] (called by name).
///
/// ## Security Features
///
/// * **Path Validation**: File paths (absolute or relative) undergo comprehensive security validation
/// * **Workspace Boundaries**: Enforces workspace directory restrictions to prevent unauthorized access
/// * **Path Traversal Protection**: Blocks dangerous path sequences like `../` to prevent directory traversal attacks
/// * **Permission Checking**: Validates read permissions before attempting file access
/// * **Audit Logging**: Logs all file access attempts for security monitoring and compliance
///
/// ## Performance Features
///
/// * **Configurable Limits**: Prevents excessive resource usage with offset/limit boundaries
/// * **Memory Efficient**: Supports partial reading of large files without loading entire content
/// * **Text Only**: Reads UTF-8 text; non-UTF-8 (binary) files are rejected with an error
/// * **Concurrent Safe**: Thread-safe operations for multiple simultaneous file reads
///
/// ## Supported Parameters
///
/// * `path`: Required path to the file to read (absolute or relative to current working directory)
/// * `offset`: Optional starting line number (1-based, max 1,000,000)
/// * `limit`: Optional maximum lines to read (1-100,000 lines)
pub async fn execute_read(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    use crate::mcp::tools::files::shared_utils::{whole_file_hash, window_lines, SecureFileAccess};
    use serde::Deserialize;
    use swissarmyhammer_common::rate_limiter::get_rate_limiter;

    tracing::debug!(
        "files read execute() called with arguments: {:?}",
        arguments
    );

    #[derive(Deserialize)]
    struct ReadRequest {
        #[serde(alias = "absolute_path", alias = "file_path")]
        path: String,
        #[serde(
            default,
            deserialize_with = "crate::mcp::tools::files::shared_utils::deserialize_flexible_usize"
        )]
        offset: Option<usize>,
        #[serde(
            default,
            deserialize_with = "crate::mcp::tools::files::shared_utils::deserialize_flexible_usize"
        )]
        limit: Option<usize>,
        #[serde(default)]
        format: Option<String>,
    }

    // Parse arguments
    let request: ReadRequest = match BaseToolImpl::parse_arguments::<ReadRequest>(arguments) {
        Ok(r) => {
            tracing::debug!(
                "Parsed request successfully: path={}, offset={:?}, limit={:?}",
                r.path,
                r.offset,
                r.limit
            );
            r
        }
        Err(e) => {
            tracing::error!("Failed to parse arguments: {}", e);
            return Err(e);
        }
    };

    // Check rate limit using tokio task ID as client identifier
    let rate_limiter = get_rate_limiter();
    let client_id = format!("task_{:?}", tokio::task::try_id());
    if let Err(e) = rate_limiter.check_rate_limit(&client_id, "file_read", 1) {
        tracing::warn!("Rate limit exceeded for file_read: {}", e);
        return Err(McpError::invalid_request(
            format!("Rate limit exceeded: {}", e),
            None,
        ));
    }

    // Validate parameters before security layer
    if let Some(offset) = request.offset {
        if offset > 1_000_000 {
            return Err(McpError::invalid_request(
                "offset must be less than 1,000,000 lines".to_string(),
                None,
            ));
        }
    }

    if let Some(limit) = request.limit {
        if limit == 0 {
            return Err(McpError::invalid_request(
                "limit must be greater than 0".to_string(),
                None,
            ));
        }
        if limit > 100_000 {
            return Err(McpError::invalid_request(
                "limit must be less than or equal to 100,000 lines".to_string(),
                None,
            ));
        }
    }

    if request.path.is_empty() {
        return Err(McpError::invalid_request(
            "path cannot be empty".to_string(),
            None,
        ));
    }

    // Resolve relative paths against the session working directory (the board
    // dir), never the process CWD.
    let session_root = context.session_root();

    // Create secure file access with enhanced security validation. It performs
    // the full path validation (absolute/relative resolution against the session
    // root, traversal and boundary checks) internally, so the request path is
    // passed through directly rather than validated a second time here.
    let secure_access = SecureFileAccess::default_secure(session_root);

    // Log file access attempt for security auditing
    info!(
        path = %request.path,
        offset = request.offset,
        limit = request.limit,
        "Attempting to read file"
    );

    let format = ReadFormat::parse(request.format.as_deref())?;

    // Read the full file once: the whole-file content is needed both for the
    // freshness token (hashed over all bytes) and for hashline tagging with
    // absolute line numbers. Windowing is applied afterward in this handler.
    let full_content = secure_access.read(&request.path, None, None)?;
    let hash = whole_file_hash(&full_content);

    // Apply offset/limit windowing to the body that is returned.
    let windowed = window_lines(&full_content, request.offset, request.limit);

    // In hashline form, tag each line `N:HH|` with the absolute 1-based line
    // number (the window starts at `offset`, defaulting to line 1) so anchors
    // stay stable across `offset`/`limit` windows. Non-UTF-8 (binary) files
    // never reach here — `read_to_string` rejects them with an error upstream —
    // so the body is always text and binary is never tagged.
    let body = match format {
        ReadFormat::Hashline => {
            let start_line = request.offset.unwrap_or(1);
            swissarmyhammer_hashline::tag(&windowed, start_line)
        }
        ReadFormat::Plain => windowed,
    };

    // Prepend the freshness-token metadata line so `write files` / `edit files`
    // can re-base against it.
    let payload = format!("{HASH_LINE_PREFIX}{hash}\n{body}");

    debug!(
        path = %request.path,
        content_length = body.len(),
        format = ?format,
        whole_file_hash = %hash,
        "Successfully read file content"
    );

    Ok(BaseToolImpl::create_success_response(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_basic_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "Hello, world!\nLine 2\nLine 3\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("Hello, world!"));
        assert!(text.contains("Line 2"));
    }

    #[tokio::test]
    async fn test_read_with_offset() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("offset_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(3));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        // Offset 3 means skip lines 1 and 2 (1-based), start from line 3
        assert!(!text.contains("Line 1"));
        assert!(!text.contains("Line 2"));
        assert!(text.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_with_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("limit_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("limit".to_string(), serde_json::json!(2));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(!text.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("offset_limit_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(2));
        args.insert("limit".to_string(), serde_json::json!(2));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        // Offset 2 means skip line 1, start from line 2, take 2 lines
        assert!(!text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(text.contains("Line 3"));
        assert!(!text.contains("Line 4"));
    }

    #[tokio::test]
    async fn test_read_with_string_offset_and_limit() {
        // Language models frequently stringify numeric arguments, sending
        // offset/limit as `"60"`/`"40"` instead of `60`/`40`. These must be
        // coerced rather than rejected with `invalid type: string, expected usize`.
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("string_args_test.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!("2"));
        args.insert("limit".to_string(), serde_json::json!("2"));

        let result = execute_read(args, &context).await;
        assert!(result.is_ok(), "string offset/limit should be accepted");
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        // Offset 2 means skip line 1, start from line 2, take 2 lines
        assert!(!text.contains("Line 1"));
        assert!(text.contains("Line 2"));
        assert!(text.contains("Line 3"));
        assert!(!text.contains("Line 4"));
    }

    #[tokio::test]
    async fn test_read_empty_path_error() {
        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert("path".to_string(), serde_json::json!(""));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("path cannot be empty") || err.contains("empty"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent = temp_dir.path().join("does_not_exist.txt");

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(nonexistent.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(
            err.contains("not found") || err.contains("NotFound") || err.contains("does not exist")
        );
    }

    #[tokio::test]
    async fn test_read_offset_exceeds_max() {
        let context = create_test_context().await;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(1_000_001));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("offset") || err.contains("1,000,000"));
    }

    #[tokio::test]
    async fn test_read_limit_zero_error() {
        let context = create_test_context().await;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("limit".to_string(), serde_json::json!(0));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("limit") || err.contains("greater than 0"));
    }

    #[tokio::test]
    async fn test_read_limit_exceeds_max() {
        let context = create_test_context().await;
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("limit".to_string(), serde_json::json!(100_001));

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("limit") || err.contains("100,000"));
    }

    #[tokio::test]
    async fn test_read_file_path_alias() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("alias_test.txt");
        fs::write(&test_file, "alias test content").unwrap();

        let context = create_test_context().await;

        // Test with file_path alias
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty.txt");
        fs::write(&test_file, "").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_read_unicode_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode.txt");
        let content = "Hello 🌍!\nЗдравствуй мир!\n你好世界\n";
        fs::write(&test_file, content).unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("🌍"));
        assert!(text.contains("Здравствуй"));
    }

    #[tokio::test]
    async fn test_read_missing_path_parameter() {
        let context = create_test_context().await;
        // No path field at all
        let args = serde_json::Map::new();

        let result = execute_read(args, &context).await;
        assert!(result.is_err());
    }

    /// Extract the text payload of a successful read result.
    fn read_text(result: &CallToolResult) -> String {
        match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text content"),
        }
    }

    /// The body of a read result, with the leading `#hash:...` metadata line
    /// removed so callers can assert on the file content alone. Preserves the
    /// body verbatim (including any trailing newline).
    fn read_body(result: &CallToolResult) -> String {
        let text = read_text(result);
        match text.split_once('\n') {
            Some((_hash_line, body)) => body.to_string(),
            None => String::new(),
        }
    }

    #[tokio::test]
    async fn test_default_output_is_hashline_tagged() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("tagged.txt");
        fs::write(&test_file, "alpha\nbeta\ngamma\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await.unwrap();
        let body = read_body(&result);

        // Each line carries an absolute, 1-based hashline anchor `N:HH|line`.
        let mut lines = body.lines();
        let expected = swissarmyhammer_hashline::tag("alpha\nbeta\ngamma\n", 1);
        assert_eq!(body, expected);
        assert!(lines.next().unwrap().starts_with("1:"));
    }

    #[tokio::test]
    async fn test_plain_format_opts_out_of_tagging() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("plain.txt");
        fs::write(&test_file, "alpha\nbeta\ngamma\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("format".to_string(), serde_json::json!("plain"));

        let result = execute_read(args, &context).await.unwrap();
        let body = read_body(&result);

        // Plain form is the pre-existing untagged content, verbatim.
        assert_eq!(body, "alpha\nbeta\ngamma\n");
        assert!(!body.contains('|'));
    }

    #[tokio::test]
    async fn test_hashline_n_is_absolute_under_offset() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("offset_tag.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(3));

        let result = execute_read(args, &context).await.unwrap();
        let body = read_body(&result);

        // The first emitted line is file line 3 — its anchor must read `3:`,
        // not `1:`, so anchors stay stable across windows.
        let first = body.lines().next().unwrap();
        assert!(
            first.starts_with("3:"),
            "expected absolute anchor 3:, got {first}"
        );
        assert!(first.ends_with("|Line 3"));
        assert!(!body.contains("Line 1"));
    }

    #[tokio::test]
    async fn test_read_result_exposes_whole_file_hash() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("hashed.txt");
        fs::write(&test_file, "alpha\nbeta\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );

        let result = execute_read(args, &context).await.unwrap();
        let text = read_text(&result);

        // The first line is a `#hash:<hex>` freshness token over full file bytes.
        let first = text.lines().next().unwrap();
        assert!(
            first.starts_with("#hash:"),
            "expected leading #hash: metadata line, got {first}"
        );
        let hash = first.strip_prefix("#hash:").unwrap();
        assert!(!hash.is_empty());
        // It matches the shared whole-file hash of the on-disk bytes.
        let expected = crate::mcp::tools::files::shared_utils::whole_file_hash("alpha\nbeta\n");
        assert_eq!(hash, expected);
    }

    #[tokio::test]
    async fn test_whole_file_hash_is_stable_across_identical_reads() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("stable.txt");
        fs::write(&test_file, "one\ntwo\nthree\n").unwrap();

        let context = create_test_context().await;
        let read_once = || {
            let mut args = serde_json::Map::new();
            args.insert(
                "path".to_string(),
                serde_json::json!(test_file.to_string_lossy()),
            );
            args
        };

        let a = execute_read(read_once(), &context).await.unwrap();
        let b = execute_read(read_once(), &context).await.unwrap();

        let hash_a = read_text(&a).lines().next().unwrap().to_string();
        let hash_b = read_text(&b).lines().next().unwrap().to_string();
        assert_eq!(hash_a, hash_b);

        // The token reflects content: a changed file yields a different hash.
        fs::write(&test_file, "one\ntwo\nCHANGED\n").unwrap();
        let c = execute_read(read_once(), &context).await.unwrap();
        let hash_c = read_text(&c).lines().next().unwrap().to_string();
        assert_ne!(hash_a, hash_c);
    }

    #[tokio::test]
    async fn test_binary_file_is_rejected_in_both_formats() {
        // The read path decodes UTF-8 (`read_to_string`); non-UTF-8 bytes are
        // rejected with an error rather than tagged or base64-encoded. This
        // holds for both formats, so binary content is never tagged.
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("binary.bin");
        // Invalid UTF-8 byte sequence.
        fs::write(&test_file, [0x00u8, 0xff, 0xfe, 0x80, 0x01]).unwrap();

        let context = create_test_context().await;
        for format in ["hashline", "plain"] {
            let mut args = serde_json::Map::new();
            args.insert(
                "path".to_string(),
                serde_json::json!(test_file.to_string_lossy()),
            );
            args.insert("format".to_string(), serde_json::json!(format));

            let result = execute_read(args, &context).await;
            assert!(
                result.is_err(),
                "binary file should be rejected with format={format}"
            );
        }
    }

    #[tokio::test]
    async fn test_hashline_offset_and_limit_anchor_matches_true_line() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("window.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let context = create_test_context().await;
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::json!(test_file.to_string_lossy()),
        );
        args.insert("offset".to_string(), serde_json::json!(2));
        args.insert("limit".to_string(), serde_json::json!(2));

        let result = execute_read(args, &context).await.unwrap();
        let body = read_body(&result);

        let anchors: Vec<&str> = body.lines().collect();
        assert_eq!(anchors.len(), 2);
        assert!(anchors[0].starts_with("2:") && anchors[0].ends_with("|Line 2"));
        assert!(anchors[1].starts_with("3:") && anchors[1].ends_with("|Line 3"));
    }
}
