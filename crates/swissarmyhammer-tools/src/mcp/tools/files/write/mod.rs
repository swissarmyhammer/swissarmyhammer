//! File writing tool for MCP operations
//!
//! This module provides the WriteFileTool for creating new files or overwriting existing files
//! with atomic operations, comprehensive security validation, and proper error handling.

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use std::path::Path;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use tracing::{debug, info};

/// Maximum size, in bytes, of content a single `write` accepts (10 MiB).
///
/// Lifted to module scope so the size-limit test can assert against the same
/// value the production path enforces, rather than re-deriving the literal.
pub(crate) const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10 MiB

/// Rate-limit token cost charged per `write` call.
///
/// A whole-file write is one logical operation, so it costs a single token in
/// the shared `"file_write"` bucket (mirrors `read`, which also costs `1`; the
/// per-edit-pair cost of `edit` is the variable case). See [`enforce_rate_limit`].
///
/// [`enforce_rate_limit`]: crate::mcp::tools::files::shared_utils::enforce_rate_limit
const FILE_WRITE_COST: u32 = 1;

/// Operation metadata for writing files
#[derive(Debug, Default)]
pub struct WriteFile;

static WRITE_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Absolute path for the new or existing file")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("content")
        .description("Complete file content to write")
        .param_type(ParamType::String)
        .required(),
];

impl Operation for WriteFile {
    fn verb(&self) -> &'static str {
        "write"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Create new files or overwrite existing files with atomic operations"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        WRITE_FILE_PARAMS
    }
}

/// Tool for creating new files or completely overwriting existing files with atomic operations
#[derive(Default)]
pub struct WriteFileTool;

impl WriteFileTool {
    /// Creates a new instance of the WriteFileTool
    pub fn new() -> Self {
        Self
    }

    /// Performs atomic file write operation using temporary file strategy
    ///
    /// This method implements the atomic write pattern:
    /// 1. Write content to temporary file with unique name in target directory
    /// 2. Atomically rename temporary file to target filename
    /// 3. Clean up temporary file on any failure
    ///
    /// The temporary file uses a ULID suffix to ensure uniqueness and avoid
    /// race conditions with concurrent writes to the same file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The target file path (already validated)
    /// * `content` - The content to write
    ///
    /// # Returns
    ///
    /// * `Result<usize, McpError>` - Number of bytes written or error
    async fn write_file_atomic(file_path: &Path, content: &str) -> Result<usize, McpError> {
        use crate::mcp::tools::files::shared_utils::{ensure_directory_exists, handle_file_error};
        use tokio::fs;
        use ulid::Ulid;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            ensure_directory_exists(parent)?;
        }

        // Create temporary file with unique name in same directory as target
        let temp_file_name = format!("{}.tmp.{}", file_path.display(), Ulid::new());
        let temp_path = Path::new(&temp_file_name);

        debug!(target_path = %file_path.display(), temp_path = %temp_path.display(), content_length = content.len(), "Starting atomic write operation");

        // Write content to the temp file, then atomically rename it onto the
        // target. Both steps share a single cleanup path: on any failure the
        // temp file is removed once before the error is surfaced.
        let write_then_rename = async {
            fs::write(temp_path, content.as_bytes())
                .await
                .map_err(|e| handle_file_error(e, "write temporary file", temp_path))?;
            fs::rename(temp_path, file_path)
                .await
                .map_err(|e| handle_file_error(e, "rename to target", file_path))?;
            Ok(())
        }
        .await;

        match write_then_rename {
            Ok(()) => {
                debug!(path = %file_path.display(), bytes_written = content.len(), "Atomic write operation completed successfully");
                Ok(content.len())
            }
            Err(e) => {
                // Single cleanup path: remove the temp file on any failure
                // (write or rename). A missing temp file is a benign no-op.
                let _ = fs::remove_file(temp_path).await;
                Err(e)
            }
        }
    }
}

/// Execute a file write operation
pub async fn execute_write(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    use crate::mcp::tools::files::shared_utils::{
        ensure_directory_exists, mutation_success_response,
    };
    use serde::Deserialize;
    use std::path::PathBuf;

    #[derive(Deserialize)]
    struct WriteRequest {
        #[serde(alias = "path", alias = "absolute_path")]
        file_path: String,
        content: String,
    }

    // Parse arguments
    let request: WriteRequest = BaseToolImpl::parse_arguments(arguments)?;

    // Check rate limit (shared helper; keyed by the current Tokio task).
    crate::mcp::tools::files::shared_utils::enforce_rate_limit("file_write", FILE_WRITE_COST)?;

    // Validate parameters
    if request.file_path.trim().is_empty() {
        return Err(McpError::invalid_request(
            "file_path cannot be empty".to_string(),
            None,
        ));
    }

    if request.content.len() > MAX_FILE_SIZE {
        return Err(McpError::invalid_request(
            "content exceeds maximum size limit of 10MB".to_string(),
            None,
        ));
    }

    // Path resolution is intentionally absolute-path-friendly: the documented
    // contract (see `description.md`, `file_path` param: "Absolute path for the
    // new or existing file") is that callers pass an absolute target, and agents
    // rely on writing to absolute paths anywhere they have OS permission. An
    // absolute path is therefore taken as-is — NOT confined to the session root.
    // A relative path is the convenience case: it resolves against the session
    // working directory (the board dir), never the process CWD.
    let path_buf = PathBuf::from(&request.file_path);
    let validated_path = if path_buf.is_absolute() {
        path_buf
    } else {
        context.session_root().join(path_buf)
    };

    // Reject `..` traversal in either form. This guards against a relative path
    // climbing out of the session root and against an absolute path smuggling a
    // `ParentDir` component; it deliberately does NOT confine absolute paths to
    // the session root, which would break the documented absolute-path contract.
    for component in validated_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(McpError::invalid_request(
                format!("Path traversal detected: {}", validated_path.display()),
                None,
            ));
        }
    }

    // Ensure parent directory exists before checking permissions
    if let Some(parent) = validated_path.parent() {
        ensure_directory_exists(parent)?;
    }

    // Check write permissions after ensuring parent directory exists
    use crate::mcp::tools::files::shared_utils::{check_file_permissions, FileOperation};
    check_file_permissions(&validated_path, FileOperation::Write)?;

    // Log file write attempt for security auditing
    info!(path = %validated_path.display(), content_length = request.content.len(), "Attempting to write file");

    // A full-file write always clobbers the target — new or existing, with the
    // same unguarded code path. There is no freshness check: whole-file
    // replacement is the whole point of `write`, and source control is the
    // recovery path. (Lost-update protection lives in line-anchored `edit
    // files`, via hashline.)

    // Perform atomic write operation
    let bytes_written = WriteFileTool::write_file_atomic(&validated_path, &request.content).await?;

    // Record the mutated path on the typed side-channel so the dispatch
    // chokepoint can fold inline diagnostics into this result (no content
    // parsing). This is DISTINCT from the `mutated_paths` carried in the result
    // body below — the side-channel drives inline diagnostics; the body surfaces
    // the paths to the model. Keep both. `validated_path` is already the absolute
    // path that was written.
    context.record_mutated_path(validated_path.clone());

    let success_message = "OK".to_string();

    debug!(path = %request.file_path, bytes_written = bytes_written, "File write operation completed successfully");

    // Carry the mutating-result envelope: the just-written content re-tagged with
    // hashline anchors (so the model can chain the next edit without re-reading)
    // plus the mutated path.
    Ok(mutation_success_response(
        success_message,
        &request.content,
        vec![validated_path.to_string_lossy().into_owned()],
        serde_json::json!({ "bytes_written": bytes_written }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// One byte over [`MAX_FILE_SIZE`], so a write of this size must be rejected.
    /// Derived from the production constant rather than re-stating the literal,
    /// so the test stays in sync if the limit ever changes.
    const TEST_FILE_SIZE_OVER_LIMIT: usize = MAX_FILE_SIZE + 1;

    /// Read-only-for-all Unix permission bits (`r--r--r--`), used to make a test
    /// fixture file reject writes.
    #[cfg(unix)]
    const READ_ONLY_PERMS: u32 = 0o444;

    /// Create test arguments for the write tool
    fn create_test_arguments(
        file_path: &str,
        content: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String(file_path.to_string()),
        );
        args.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        args
    }

    /// Assert no leftover `*.tmp.*` files remain in `parent_dir` — the atomic
    /// write must clean up its temporary file on success and on every failure.
    fn assert_no_temp_files_remain(parent_dir: &Path) {
        let temp_files: Vec<_> = fs::read_dir(parent_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(
            temp_files.is_empty(),
            "temporary files should be cleaned up, found: {temp_files:?}"
        );
    }

    /// Set `path` to read-only permissions so a subsequent write fails. No-op on
    /// non-Unix targets (the read-only-rename behavior is Unix-specific here).
    #[cfg(unix)]
    fn make_file_readonly(path: &Path) {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, Permissions::from_mode(READ_ONLY_PERMS)).unwrap();
    }
    #[cfg(not(unix))]
    fn make_file_readonly(_path: &Path) {}

    #[test]
    fn test_write_tool_creation() {
        let op = WriteFile;
        assert_eq!(op.verb(), "write");
        assert_eq!(op.noun(), "file");
        assert!(!op.description().is_empty());
    }

    #[tokio::test]
    async fn test_write_new_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_new_file.txt");
        let test_content = "Hello, World!\nThis is a test file.";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), test_content);

        let call_result = execute_write(args, &context).await.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was created with correct content
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, test_content);
    }

    #[tokio::test]
    async fn test_write_overwrite_existing_file() {
        // A full-file write clobbers an existing file unconditionally — no
        // freshness token, no hash check. Source control is the safety net.
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_overwrite.txt");

        // Create initial file
        let initial_content = "Initial content";
        fs::write(&test_file, initial_content).unwrap();
        assert_eq!(fs::read_to_string(&test_file).unwrap(), initial_content);

        // Overwrite with new content, with NO expected_hash supplied.
        let new_content = "New content that replaces the old";
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), new_content);

        let call_result = execute_write(args, &context).await.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was overwritten on disk.
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, new_content);
        assert_ne!(written_content, initial_content);

        // …and the overwrite carries the normal mutation envelope.
        let structured = call_result
            .structured_content
            .expect("successful overwrite sets structured content");
        let mutation = &structured["mutation"];
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            swissarmyhammer_hashline::tag(new_content, 1)
        );
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("test_overwrite.txt"));
        assert!(mutation["bytes_written"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_write_creates_parent_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_file = temp_dir
            .path()
            .join("deeply")
            .join("nested")
            .join("directory")
            .join("test.txt");
        let test_content = "Content in nested directory";

        assert!(!nested_file.parent().unwrap().exists());

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&nested_file.to_string_lossy(), test_content);

        let result = execute_write(args, &context).await;
        assert!(result.is_ok());

        // Verify parent directories were created
        assert!(nested_file.parent().unwrap().exists());
        assert!(nested_file.exists());

        let written_content = fs::read_to_string(&nested_file).unwrap();
        assert_eq!(written_content, test_content);
    }

    #[tokio::test]
    async fn test_write_empty_file_path() {
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments("", "test content");

        let result = execute_write(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("file_path cannot be empty"));
    }

    #[tokio::test]
    async fn test_write_whitespace_file_path() {
        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments("   ", "test content");

        let result = execute_write(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("file_path cannot be empty"));
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_write_relative_path_acceptance() {
        use swissarmyhammer_common::test_utils::CurrentDirGuard;

        let temp_dir = TempDir::new().unwrap();
        // The RAII guard pins cwd to the temp dir for the whole test and
        // restores the original working directory on drop, even on panic.
        let _cwd_guard = CurrentDirGuard::new(temp_dir.path())
            .expect("Failed to pin working directory to the isolated temp dir");

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments("relative_file.txt", "test content");

        let result = execute_write(args, &context).await;
        assert!(result.is_ok(), "Relative paths should now be accepted");

        // Verify file was created
        let file_path = temp_dir.path().join("relative_file.txt");
        assert!(file_path.exists(), "File should have been created");

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn test_write_content_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");

        // One byte over the limit, derived from MAX_FILE_SIZE via the test constant.
        let large_content = "x".repeat(TEST_FILE_SIZE_OVER_LIMIT);

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), &large_content);

        let result = execute_write(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("exceeds maximum size limit"));
    }

    #[tokio::test]
    async fn test_write_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode_test.txt");
        let unicode_content = "Hello 🦀 Rust!\n你好世界\nПривет мир\n🚀✨🎉";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), unicode_content);

        let result = execute_write(args, &context).await;
        assert!(result.is_ok());

        // Verify Unicode content was written correctly
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, unicode_content);
    }

    #[tokio::test]
    async fn test_write_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_file.txt");
        let empty_content = "";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), empty_content);

        let result = execute_write(args, &context).await;
        assert!(result.is_ok());

        // Verify empty file was created
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, empty_content);

        let metadata = fs::metadata(&test_file).unwrap();
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    async fn test_atomic_write_operation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("atomic_test.txt");
        let test_content = "Atomic write test content";

        // Test that the atomic write method works correctly
        let result = WriteFileTool::write_file_atomic(&test_file, test_content).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_content.len());

        // Verify file exists and has correct content
        assert!(test_file.exists());
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, test_content);

        // Verify no temporary files remain.
        assert_no_temp_files_remain(test_file.parent().unwrap());
    }

    #[tokio::test]
    async fn test_atomic_write_cleanup_on_failure() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readonly_test.txt");

        // Create a read-only file that should cause rename to fail
        fs::write(&test_file, "existing content").unwrap();
        make_file_readonly(&test_file);

        let test_content = "This should fail to write";

        // The atomic write should fail but clean up temporary file
        let _result = WriteFileTool::write_file_atomic(&test_file, test_content).await;

        // Note: This test may pass on some systems where rename succeeds despite
        // a readonly target. The key invariant is that the temporary file is
        // cleaned up regardless of whether the rename succeeded or failed.
        assert_no_temp_files_remain(test_file.parent().unwrap());
    }

    /// `WriteFileTool::new()` and the derived `Default` produce the same unit
    /// value — the public constructor is equivalent to `default()`.
    #[test]
    fn test_write_file_tool_new_equals_default() {
        let _new = WriteFileTool::new();
        let _default = WriteFileTool;
        // Unit struct: construction simply must succeed via both paths.
    }

    /// The atomic-write rename step can itself fail (the temp file was written
    /// fine, but the rename onto the target cannot complete). When the target
    /// path is an existing **directory**, renaming a regular temp file over it
    /// fails — exercising the rename-failure cleanup arm. The temp file must be
    /// removed and an error surfaced.
    #[tokio::test]
    async fn test_atomic_write_cleanup_on_rename_failure() {
        let temp_dir = TempDir::new().unwrap();
        // Target is a directory: fs::rename(temp_file, dir) fails.
        let target_dir = temp_dir.path().join("i_am_a_directory");
        fs::create_dir(&target_dir).unwrap();

        let result = WriteFileTool::write_file_atomic(&target_dir, "payload").await;
        assert!(
            result.is_err(),
            "renaming a temp file over an existing directory must fail"
        );

        // The directory must be untouched (still a directory, still empty).
        assert!(target_dir.is_dir());
        assert_eq!(fs::read_dir(&target_dir).unwrap().count(), 0);

        // No leftover temp files in the parent.
        assert_no_temp_files_remain(temp_dir.path());
    }

    #[tokio::test]
    async fn test_write_file_with_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("special_chars.txt");
        let special_content =
            "Line 1\nLine 2\r\nTab\tcharacter\nNull: \0 (null byte)\nBackslash: \\ forward: /";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), special_content);

        let result = execute_write(args, &context).await;
        assert!(result.is_ok());

        // Verify special characters were written correctly
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, special_content);
    }

    #[tokio::test]
    async fn test_write_json_argument_parsing_error() {
        let context = crate::test_utils::create_test_context().await;

        // Create invalid arguments (missing required field)
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(),
            serde_json::Value::String("/test/path".to_string()),
        );
        // Missing "content" field

        let result = execute_write(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(format!("{:?}", error).contains("Invalid arguments"));
    }

    #[tokio::test]
    async fn test_write_success_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("response_test.txt");
        let test_content = "Testing response format";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), test_content);

        let result = execute_write(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());

        // The first content block stays the plain "OK" success message.
        let response_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content in response"),
        };
        assert_eq!(response_text, "OK");

        // …and a successful write now also carries the mutating-result envelope:
        // the hashline-tagged content just written and the mutated path. Verify
        // the write really happened, then assert the envelope describes it.
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            test_content,
            "the write must have been committed"
        );
        let structured = call_result
            .structured_content
            .expect("successful write sets structured content");
        let mutation = &structured["mutation"];
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            swissarmyhammer_hashline::tag(test_content, 1)
        );
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("response_test.txt"));
    }

    #[tokio::test]
    async fn test_write_readonly_file_fails() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readonly_file.txt");

        // Create a file and make it read-only
        fs::write(&test_file, "initial content").unwrap();
        make_file_readonly(&test_file);

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), "new content");

        let result = execute_write(args, &context).await;
        assert!(result.is_err(), "Writing to read-only file should fail");

        let error = result.unwrap_err();
        let error_message = format!("{:?}", error);
        assert!(
            error_message.contains("read-only") || error_message.contains("readonly"),
            "Error should mention read-only permission: {}",
            error_message
        );
    }

    #[tokio::test]
    async fn test_write_new_file_is_unguarded() {
        // A brand-new (nonexistent) file writes freely with no token — the same
        // unguarded path a full-file write always takes.
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("guard_new.txt");
        assert!(!test_file.exists());

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), "fresh content");

        let result = execute_write(args, &context).await;
        assert!(result.is_ok(), "new-file write should succeed unguarded");
        assert_eq!(result.unwrap().is_error, Some(false));

        assert_eq!(fs::read_to_string(&test_file).unwrap(), "fresh content");
    }

    // --- Mutating-result envelope: tagged_content + mutated_paths ------------

    /// A successful write (here a brand-new file) carries the mutation envelope:
    /// `tagged_content` (hashline-tagged content just written) + `mutated_paths`
    /// in the structured surface, plus an appended text block; the first content
    /// block stays the plain "OK".
    #[tokio::test]
    async fn successful_write_carries_tagged_content_and_mutated_paths() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("write_envelope.txt");
        let content = "first\nsecond\nthird\n";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), content);

        let call = execute_write(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));

        // First block stays the plain success message.
        match &call.content[0].raw {
            rmcp::model::RawContent::Text(t) => assert_eq!(t.text, "OK"),
            _ => panic!("expected text content"),
        }

        let structured = call
            .structured_content
            .clone()
            .expect("successful write sets structured content");
        let mutation = &structured["mutation"];
        let expected_tagged = swissarmyhammer_hashline::tag(content, 1);
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            expected_tagged
        );
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("write_envelope.txt"));
        assert!(mutation["bytes_written"].as_u64().unwrap() > 0);

        let all_text = call
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            all_text.contains(&expected_tagged),
            "envelope text block carries the tagged content"
        );
    }

    /// Round-trip: an anchor taken from a successful write's `tagged_content`
    /// resolves against the on-disk file in an immediately-following `edit files`
    /// call, with NO intervening read.
    #[tokio::test]
    async fn anchor_from_write_envelope_resolves_in_edit() {
        use crate::mcp::tools::files::edit::execute_edit;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("write_roundtrip.txt");
        let content = "one\ntwo\nthree\n";

        let context = crate::test_utils::create_test_context().await;
        let args = create_test_arguments(&test_file.to_string_lossy(), content);
        let call = execute_write(args, &context).await.unwrap();
        let structured = call.structured_content.expect("structured content");
        let tagged = structured["mutation"]["tagged_content"]
            .as_str()
            .unwrap()
            .to_string();

        // Pull the anchor for line 2 (two) directly from tagged_content.
        let anchor = tagged
            .lines()
            .find(|l| l.contains("|two"))
            .and_then(|l| l.split('|').next())
            .expect("two line present")
            .to_string();
        assert!(anchor.starts_with("2:"), "anchor targets line 2: {anchor}");

        let mut edit_args = serde_json::Map::new();
        edit_args.insert(
            "file_path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        edit_args.insert("find".to_string(), serde_json::Value::String(anchor));
        edit_args.insert(
            "replace".to_string(),
            serde_json::Value::String("TWO".to_string()),
        );

        let edit_call = execute_edit(edit_args, &context).await.unwrap();
        assert_eq!(edit_call.is_error, Some(false), "anchor must resolve");
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "one\nTWO\nthree\n");
    }
}
