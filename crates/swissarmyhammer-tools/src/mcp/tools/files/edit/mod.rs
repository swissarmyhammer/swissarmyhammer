// sah rule ignore acp/capability-enforcement
//! File editing tool for MCP operations
//!
//! This module provides the EditFileTool for performing precise string replacements in files
//! with atomic operations, comprehensive security validation, file encoding preservation,
//! and metadata preservation.
//!
//! The operation is split across four submodules, each one rung of the same
//! pipeline:
//!
//! - [`args`] turns the forgiving argument surface into canonical [`EditPair`]s.
//! - [`cascade`] resolves each pair against the file content and applies the
//!   whole batch to an in-memory copy.
//! - [`prompts`] renders the ambiguity / near-miss / already-applied answers the
//!   cascade returns in place of an error.
//! - [`atomic`] commits the rewritten content to disk in one atomic rewrite.
//!
//! [`execute_edit`] below is the entry point that drives all four.
//!
//! Note: This is an MCP tool, not an ACP operation. ACP capability checking happens at the
//! agent layer (claude-agent), not at the MCP tool layer.

mod args;
mod atomic;
mod cascade;
mod prompts;
#[cfg(test)]
mod test_support;

pub use args::{looks_like_edit, normalize_edit_args, EditPair};
pub use atomic::{EditFileTool, EditResult};

use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{Operation, ParamMeta};
use tracing::{debug, info};

use args::{first_present, EDIT_FILE_PARAMS, FILE_PATH_ALIASES, FIND_ALIASES, REPLACE_ALIASES};
use atomic::LineEnding;
use cascade::{apply_all_pairs, ApplyOutcome};
use prompts::{
    render_already_applied_prompt, render_ambiguity_prompt, render_consumed_target_prompt,
    render_near_miss_prompt,
};

/// Operation metadata for editing files
#[derive(Debug, Default)]
pub struct EditFile;

impl Operation for EditFile {
    fn verb(&self) -> &'static str {
        "edit"
    }
    fn noun(&self) -> &'static str {
        "file"
    }
    fn description(&self) -> &'static str {
        "Perform precise string replacements in existing files"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        EDIT_FILE_PARAMS
    }
}

/// Execute a file edit operation
pub async fn execute_edit(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    // Extract file path under any canonical/alias key.
    let file_path = first_present(&arguments, "file_path", FILE_PATH_ALIASES)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            McpError::invalid_request("path/file_path/filePath is required".to_string(), None)
        })?
        .to_string();

    // Validate file path
    if file_path.trim().is_empty() {
        return Err(McpError::invalid_request(
            "path cannot be empty".to_string(),
            None,
        ));
    }

    // An explicitly empty `edits: []` (with no top-level find/replace) keeps its
    // historical, more specific error message.
    if let Some(serde_json::Value::Array(edits)) = arguments.get("edits") {
        if edits.is_empty()
            && first_present(&arguments, "find", FIND_ALIASES).is_none()
            && first_present(&arguments, "replace", REPLACE_ALIASES).is_none()
        {
            return Err(McpError::invalid_request(
                "edits array cannot be empty".to_string(),
                None,
            ));
        }
    }

    // Normalize every accepted input shape into canonical (find, replace) pairs.
    let edit_operations = normalize_edit_args(&arguments)?;

    // Check rate limit, costed by the number of edit operations (shared helper).
    let cost = edit_operations.len() as u32;
    crate::mcp::tools::files::shared_utils::enforce_rate_limit("file_edit", cost)?;

    // Validate all edit operations
    for (idx, edit_op) in edit_operations.iter().enumerate() {
        if edit_op.find.is_empty() {
            return Err(McpError::invalid_request(
                format!("edit operation {}: old_text cannot be empty", idx),
                None,
            ));
        }

        // No-op rejection: `find == replace` would change nothing. Reject it up
        // front with a clear message — this is the single, coherent home for the
        // no-op concept (the historical "must be different" check IS the no-op
        // rejection, not a separate code path).
        if edit_op.find == edit_op.replace {
            return Err(McpError::invalid_request(
                format!(
                    "edit operation {idx}: no-op edit — `find` and `replace` are identical, so \
                     they must be different"
                ),
                None,
            ));
        }
    }

    // Log edit attempt for security auditing
    info!(path = %file_path, num_operations = edit_operations.len(), "Attempting atomic edit operation(s)");

    // Apply the whole batch atomically: read the file once, resolve and apply
    // every pair against an in-memory working copy, then commit in ONE rewrite.
    // A failure on any pair leaves the file byte-identical. Relative paths
    // resolve against the session working directory (the board dir), never the
    // process CWD.
    use crate::mcp::tools::files::shared_utils::{mutation_success_response, validate_file_path};
    let base_dir = context.session_root();
    let tool = EditFileTool::new();

    // Resolve and validate the target path (existence) once.
    let path = validate_file_path(&base_dir, &file_path)?;
    if !path.exists() {
        return Err(McpError::invalid_request(
            format!("file does not exist: {}", file_path),
            None,
        ));
    }

    // Read once with encoding detection and detect the line-ending convention.
    let (original_content, detected_encoding) = tool.read_with_encoding_detection(&path)?;
    let line_ending = LineEnding::detect(&original_content);

    // Resolve + apply every pair against the working copy (no IO). The cascade
    // (anchor → literal substring → recovery ladder) runs here.
    let new_content = match apply_all_pairs(&original_content, &edit_operations)? {
        ApplyOutcome::Applied(content) => content,
        // Ambiguity is a SUCCESSFUL result describing the choice — NOT an error.
        // Nothing was committed, so the file is byte-identical; the model retries
        // with an `occurrence` hint.
        ApplyOutcome::Ambiguous { find, candidates } => {
            info!(path = %file_path, candidate_count = candidates.len(), "Edit `find` is ambiguous; returning candidates for disambiguation");
            return Ok(BaseToolImpl::create_success_response(
                render_ambiguity_prompt(&find, &candidates),
            ));
        }
        // No confident match is a SUCCESSFUL near-miss describing how the `find`
        // diverged — NOT an error. Nothing was committed, so the file is
        // byte-identical; the model retries with corrected text.
        ApplyOutcome::NoMatch { find, near } => {
            info!(path = %file_path, near_miss_count = near.len(), "Edit `find` matched nothing confidently; returning near-misses");
            return Ok(BaseToolImpl::create_success_response(
                render_near_miss_prompt(&find, &near),
            ));
        }
        // `find` absent but `replace` already present: the edit was likely already
        // applied. Informational SUCCESS — nothing committed, file byte-identical.
        ApplyOutcome::AlreadyApplied { find, replace } => {
            info!(path = %file_path, "Edit `find` absent but `replace` present; reporting likely-already-applied");
            return Ok(BaseToolImpl::create_success_response(
                render_already_applied_prompt(&find, &replace),
            ));
        }
        // A later pair's target span was consumed by an earlier pair in this same
        // batch. Per-edit SUCCESS — nothing committed, file byte-identical.
        ApplyOutcome::ConsumedTarget { find, line } => {
            info!(path = %file_path, consumed_line = line, "Edit `find` target was consumed by an earlier edit in the batch");
            return Ok(BaseToolImpl::create_success_response(
                render_consumed_target_prompt(&find, line),
            ));
        }
    };

    // Commit the fully-edited content in one atomic rewrite.
    let final_result = tool.commit_content(
        &path,
        &new_content,
        detected_encoding,
        line_ending,
        edit_operations.len(),
    )?;
    let total_replacements = edit_operations.len();

    // Create success response
    let success_message = if edit_operations.len() == 1 {
        "OK".to_string()
    } else {
        format!("OK: Applied {} edit operations", edit_operations.len())
    };

    debug!(path = %file_path, num_operations = edit_operations.len(), bytes_written = final_result.bytes_written, total_replacements = total_replacements, encoding = %final_result.encoding_detected, line_endings = %final_result.line_endings_preserved, "Edit operation(s) completed successfully"
    );

    // Carry the mutating-result envelope: the post-edit file re-tagged with
    // hashline anchors (so the model can chain the next edit without re-reading)
    // plus the mutated path, layered on top of the existing typed EditResult
    // fields. ONLY this committed/Applied path carries the envelope — the
    // ambiguity and near-miss returns above did not mutate, so they do not.
    Ok(mutation_success_response(
        success_message,
        &new_content,
        vec![path.to_string_lossy().into_owned()],
        serde_json::json!({
            "bytes_written": final_result.bytes_written,
            "replacements_made": final_result.replacements_made,
            "encoding_detected": final_result.encoding_detected,
            "line_endings_preserved": final_result.line_endings_preserved,
        }),
    ))
}

/// End-to-end tests for the `edit` operation as a whole.
///
/// Each submodule tests its own rung; these drive [`execute_edit`] itself — the
/// argument surface it accepts, the file it leaves behind, and the mutation
/// envelope it returns.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::files::edit::test_support::{
        ambiguity_args, create_edit_arguments, result_text,
    };
    use crate::mcp::tools::files::shared_utils::TEMP_FILE_SUFFIX;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_edit_tool_operation_metadata() {
        let op = EditFile;
        assert_eq!(op.verb(), "edit");
        assert_eq!(op.noun(), "file");
        assert!(!op.description().is_empty());
    }

    #[tokio::test]
    async fn test_edit_single_replacement_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_edit.txt");
        let initial_content = "Hello world! This is a test file.";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // Verify file was edited correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a test file.");
    }

    #[tokio::test]
    async fn test_edit_replace_all_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_replace_all.txt");
        let initial_content = "test test test";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "test", "exam", Some(true));

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify all occurrences were replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam");
    }

    #[tokio::test]
    async fn test_edit_multiple_occurrences_without_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_multiple.txt");
        let initial_content = "duplicate duplicate duplicate";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "duplicate",
            "unique",
            None, // replace_all = false by default
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify only the first occurrence was replaced
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "unique duplicate duplicate");
    }

    /// A `find` with no confident match no longer errors with the bare
    /// "not found in file" string: it returns a SUCCESSFUL structured near-miss
    /// (echoing the searched-for text) and leaves the file byte-identical. Here
    /// the lone line is too dissimilar to surface as a near-miss, so the prompt
    /// states nothing is close — but it is still a successful structured result.
    #[tokio::test]
    async fn test_edit_string_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_not_found.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "no-match must be a successful structured near-miss, got {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(
            call.is_error,
            Some(false),
            "near-miss is not an error result"
        );

        let text = result_text(&call);
        // Echoes the searched-for text and is NOT the legacy "not found in file".
        assert!(text.contains("nonexistent"), "must echo the find: {text}");
        assert!(
            !text.contains("not found in file"),
            "legacy bare error string must be gone: {text}"
        );

        // Verify file was not modified
        let unchanged_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(unchanged_content, initial_content);
    }

    #[tokio::test]
    async fn test_edit_file_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_file = temp_dir.path().join("does_not_exist.txt");

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&nonexistent_file.to_string_lossy(), "old", "new", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // The error message from shared_utils says "File not found"
        assert!(
            error_str.contains("file does not exist")
                || error_str.contains("file not found")
                || error_str.contains("does not exist")
                || error_str.contains("NotFound")
        );
    }

    #[tokio::test]
    async fn test_edit_empty_parameters() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test empty file path
        let args = create_edit_arguments("", "old", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("path cannot be empty"));

        // Test empty old_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("old_text cannot be empty"));

        // Test identical old_string and new_string
        let args = create_edit_arguments(&test_file.to_string_lossy(), "same", "same", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("must be different"));
    }

    #[tokio::test]
    async fn test_edit_unicode_content() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("unicode_test.txt");
        let unicode_content = "Hello 🌍! Здравствуй мир! 你好世界!";
        fs::write(&test_file, unicode_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "🌍", "🚀", None);

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify Unicode replacement worked correctly
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello 🚀! Здравствуй мир! 你好世界!");
    }

    #[tokio::test]
    async fn test_edit_preserves_line_endings() {
        let temp_dir = TempDir::new().unwrap();

        // Test Windows line endings preservation
        let windows_file = temp_dir.path().join("windows_endings.txt");
        let windows_content = "Line 1\r\nold text\r\nLine 3\r\n";
        fs::write(&windows_file, windows_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &windows_file.to_string_lossy(),
            "old text",
            "new text",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&windows_file).unwrap();
        assert_eq!(edited_content, "Line 1\r\nnew text\r\nLine 3\r\n");
        assert!(edited_content.contains("\r\n"));
    }

    #[tokio::test]
    async fn test_edit_atomic_operation_failure_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test_atomic.txt");
        let initial_content = "original content";
        fs::write(&test_file, initial_content).unwrap();

        // Make file read-only to cause atomic operation to fail during permission setting
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;

            let readonly_permissions = Permissions::from_mode(0o444);
            fs::set_permissions(&test_file, readonly_permissions).unwrap();

            let tool = EditFileTool::new();

            // Even if the operation fails, we should verify no temporary files are left behind
            let _temp_pattern = format!("{}{TEMP_FILE_SUFFIX}*", test_file.display());

            // The edit should work even with readonly file since we change permissions on temp file
            let edit_result = tool.edit_file_atomic(
                temp_dir.path(),
                &test_file.to_string_lossy(),
                "original",
                "modified",
                false,
            );

            // Check that no temporary files remain regardless of result
            let temp_files: Vec<_> = temp_dir
                .path()
                .read_dir()
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .contains(TEMP_FILE_SUFFIX)
                })
                .collect();

            assert!(
                temp_files.is_empty(),
                "Temporary files should be cleaned up"
            );

            // If the edit succeeded, verify the content was actually changed
            if edit_result.is_ok() {
                let final_content = fs::read_to_string(&test_file).unwrap();
                assert_eq!(final_content, "modified content");
            }
        }
    }

    #[tokio::test]
    async fn test_edit_file_permissions_preservation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("permissions_test.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        // Set specific permissions (only on Unix systems)
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;

            let permissions = Permissions::from_mode(0o755);
            fs::set_permissions(&test_file, permissions).unwrap();

            let original_metadata = fs::metadata(&test_file).unwrap();
            let original_mode = original_metadata.permissions().mode();

            let tool = EditFileTool::new();
            let edit_result = tool.edit_file_atomic(
                temp_dir.path(),
                &test_file.to_string_lossy(),
                "test",
                "updated",
                false,
            );

            assert!(edit_result.is_ok());

            // Verify permissions were preserved
            let new_metadata = fs::metadata(&test_file).unwrap();
            let new_mode = new_metadata.permissions().mode();
            assert_eq!(
                original_mode, new_mode,
                "File permissions should be preserved"
            );

            // Verify content was updated
            let final_content = fs::read_to_string(&test_file).unwrap();
            assert_eq!(final_content, "updated content");
        }
    }

    /// Editing a file IS modifying it: the post-edit modification time must
    /// advance past the pre-edit mtime. Preserving the old mtime defeats every
    /// mtime-based staleness check downstream (cargo/make rebuilds, file
    /// watchers, rust-analyzer). Seed a fixed past mtime (no wall-clock sleep)
    /// and assert the edit produces a strictly greater mtime.
    #[tokio::test]
    async fn test_edit_file_advances_modification_time() {
        use filetime::{set_file_mtime, FileTime};

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("mtime_test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Seed a clearly-old mtime well in the past (2001-09-09 01:46:40 UTC).
        let old_mtime = FileTime::from_unix_time(1_000_000_000, 0);
        set_file_mtime(&test_file, old_mtime).unwrap();
        let seeded = FileTime::from_last_modification_time(&fs::metadata(&test_file).unwrap());
        assert_eq!(seeded, old_mtime, "mtime seed should be applied");

        let tool = EditFileTool::new();
        let edit_result = tool.edit_file_atomic(
            temp_dir.path(),
            &test_file.to_string_lossy(),
            "test",
            "updated",
            false,
        );
        assert!(edit_result.is_ok());

        let new_mtime = FileTime::from_last_modification_time(&fs::metadata(&test_file).unwrap());
        assert!(
            new_mtime > old_mtime,
            "edit must advance the file's modification time \
             (old={old_mtime:?}, new={new_mtime:?})"
        );
    }

    #[tokio::test]
    async fn test_edit_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("response_test.txt");
        let initial_content = "Hello world!";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let result = execute_edit(args, &context).await;
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

        // …and a successful edit now also carries the mutating-result envelope:
        // the hashline-tagged post-edit content and the mutated path. Verify the
        // mutation really happened, then assert the envelope describes it.
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "Hello universe!",
            "the edit must have been committed"
        );
        let structured = call_result
            .structured_content
            .expect("successful edit sets structured content");
        let mutation = &structured["mutation"];
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            swissarmyhammer_hashline::tag("Hello universe!", 1)
        );
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("response_test.txt"));
        assert_eq!(mutation["replacements_made"], serde_json::json!(1));
    }

    #[tokio::test]
    async fn test_edit_json_argument_parsing_error() {
        let context = crate::test_utils::create_test_context().await;

        // Create invalid arguments (missing both single edit and multiple edits modes)
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String("/test/path".to_string()),
        );
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        // Missing "new_string" field and no "edits" array

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_str = format!("{:?}", error);
        // A find (old_string is now an alias of `find`) with no matching replace
        // must error rather than silently dropping the unpaired find.
        assert!(
            error_str.contains("find provided without a matching replace")
                || error_str.contains("replace"),
            "unexpected error: {error_str}"
        );
    }

    #[tokio::test]
    async fn test_edit_large_file_handling() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_file.txt");

        // Create a reasonably large file (1MB) with repetitive content
        let chunk = "This is a line of test content that will be repeated many times.\n";
        let chunk_size = chunk.len();
        let target_size = 1_000_000; // 1MB
        let repetitions = target_size / chunk_size;

        let large_content = chunk.repeat(repetitions);
        fs::write(&test_file, &large_content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "test content",
            "modified content",
            Some(true), // Replace all occurrences
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify the replacements were made
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert!(edited_content.contains("modified content"));
        assert!(!edited_content.contains("test content"));
    }

    /// An empty file has no lines to surface, so the near-miss has no candidate
    /// spans — but it is still a SUCCESSFUL structured result (not an error) that
    /// echoes the searched-for text and states the file has nothing close.
    #[tokio::test]
    async fn test_edit_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_file.txt");
        fs::write(&test_file, "").unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "nonexistent",
            "replacement",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(
            result.is_ok(),
            "empty-file no-match must be a successful near-miss, got {result:?}"
        );
        let call = result.unwrap();
        assert_eq!(call.is_error, Some(false));

        let text = result_text(&call);
        assert!(text.contains("nonexistent"), "must echo the find: {text}");
        // No near-miss spans exist in an empty file.
        assert!(
            text.contains("no close") || text.contains("nothing close"),
            "must state nothing is close: {text}"
        );

        // File still empty (byte-identical).
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_sequential() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multiple_edits.txt");
        let initial_content = "Hello world! This is a test.";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Create arguments with multiple edits
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "oldText": "world",
                    "newText": "universe"
                },
                {
                    "oldText": "test",
                    "newText": "example"
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        // Verify all edits were applied sequentially
        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a example.");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_with_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("alias_test.txt");
        let initial_content = "foo bar baz";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test different parameter aliases
        let mut args = serde_json::Map::new();
        args.insert(
            "filePath".to_string(), // Using filePath alias
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "old_string": "foo",  // Using old_string alias
                    "new_text": "FOO"     // Using new_text alias
                },
                {
                    "old_text": "bar",    // Using old_text alias
                    "new_string": "BAR"   // Using new_string alias
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "FOO BAR baz");
    }

    #[tokio::test]
    async fn test_edit_single_mode_with_path_alias() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("single_alias.txt");
        let initial_content = "test content";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Test single edit mode with different parameter aliases
        let mut args = serde_json::Map::new();
        args.insert(
            "file_path".to_string(), // Using file_path alias
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "oldText".to_string(), // Using oldText alias
            serde_json::Value::String("test".to_string()),
        );
        args.insert(
            "newText".to_string(), // Using newText alias
            serde_json::Value::String("demo".to_string()),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "demo content");
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_with_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("replace_all_multi.txt");
        let initial_content = "test test test, example example";
        fs::write(&test_file, initial_content).unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                {
                    "oldText": "test",
                    "newText": "exam",
                    "replace_all": true
                },
                {
                    "oldText": "example",
                    "newText": "sample",
                    "replace_all": true
                }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam, sample sample");
    }

    #[tokio::test]
    async fn test_edit_empty_edits_array() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty_edits.txt");
        fs::write(&test_file, "content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert("edits".to_string(), serde_json::json!([]));

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("edits array cannot be empty"));
    }

    #[tokio::test]
    async fn test_edit_missing_path() {
        let context = crate::test_utils::create_test_context().await;

        // Missing path parameter
        let mut args = serde_json::Map::new();
        args.insert(
            "old_string".to_string(),
            serde_json::Value::String("old".to_string()),
        );
        args.insert(
            "new_string".to_string(),
            serde_json::Value::String("new".to_string()),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("path"));
    }

    #[tokio::test]
    async fn test_edit_whitespace_path_error() {
        let context = crate::test_utils::create_test_context().await;

        let args = create_edit_arguments("   ", "old", "new", None);
        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        assert!(
            format!("{:?}", result).contains("empty") || format!("{:?}", result).contains("path")
        );
    }

    #[tokio::test]
    async fn test_edit_old_string_in_index_one_operation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("index_test.txt");
        fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Multiple edits - second operation has empty old_text
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "line 1", "newText": "LINE ONE" },
                { "oldText": "", "newText": "something" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("old_text cannot be empty") || err.contains("empty"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_same_and_different_not_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("same_test.txt");
        fs::write(&test_file, "content").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // Multiple edits - second operation has same old and new text
        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "content", "newText": "new_content" },
                { "oldText": "same_text", "newText": "same_text" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("must be different") || err.contains("different"));
    }

    #[tokio::test]
    async fn test_edit_multiple_edits_success_response_format() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multi_response.txt");
        fs::write(&test_file, "foo bar baz").unwrap();

        let context = crate::test_utils::create_test_context().await;

        let mut args = serde_json::Map::new();
        args.insert(
            "path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args.insert(
            "edits".to_string(),
            serde_json::json!([
                { "oldText": "foo", "newText": "FOO" },
                { "oldText": "bar", "newText": "BAR" }
            ]),
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());
        let call_result = result.unwrap();
        let text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("Expected text"),
        };
        // Multiple edits response says "OK: Applied N edit operations"
        assert!(text.contains("OK") && text.contains("2") || text.contains("Applied"));
    }

    #[tokio::test]
    async fn test_edit_cr_line_endings_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("cr_endings.txt");
        // Classic Mac line endings
        let content = "line1\rold content\rline3\r";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "old content",
            "new content",
            None,
        );

        let result = execute_edit(args, &context).await;
        assert!(result.is_ok());

        let edited = fs::read(&test_file).unwrap();
        let edited_str = String::from_utf8(edited).unwrap();
        assert!(edited_str.contains("new content"));
        // CR line endings should be preserved
        assert!(edited_str.contains('\r'));
    }

    // =========================================================================
    // Mutating-result envelope: tagged_content + mutated_paths on SUCCESS only
    // =========================================================================

    /// Join every text content block of a result (the success message block AND
    /// the appended envelope block), so envelope assertions can scan the whole
    /// surfaced text — not just `content[0]`.
    fn all_text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A successful single-pair edit carries the mutation envelope:
    /// `tagged_content` (the hashline-tagged post-edit file) and `mutated_paths`
    /// in the structured surface, plus an appended text block, while the first
    /// content block stays the plain "OK" message.
    #[tokio::test]
    async fn successful_edit_carries_tagged_content_and_mutated_paths() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("envelope.txt");
        fs::write(&test_file, "Hello world!").unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(&test_file.to_string_lossy(), "world", "universe", None);

        let call = execute_edit(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));

        // The first block is still the plain success message.
        assert_eq!(result_text(&call), "OK");

        // Structured surface carries the envelope.
        let structured = call
            .structured_content
            .clone()
            .expect("successful edit sets structured content");
        let mutation = &structured["mutation"];
        // tagged_content is the hashline tag of the POST-edit file.
        let expected_tagged = swissarmyhammer_hashline::tag("Hello universe!", 1);
        assert_eq!(
            mutation["tagged_content"].as_str().unwrap(),
            expected_tagged
        );
        // mutated_paths carries the absolute path that was changed.
        let paths = mutation["mutated_paths"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].as_str().unwrap().ends_with("envelope.txt"));
        // Existing EditResult fields are preserved in the structured surface.
        assert_eq!(mutation["replacements_made"], serde_json::json!(1));
        assert!(mutation["bytes_written"].as_u64().unwrap() > 0);
        assert!(mutation.get("encoding_detected").is_some());
        assert!(mutation.get("line_endings_preserved").is_some());

        // The appended text block also carries the tagged content so text-only
        // hosts deliver it to the model.
        assert!(
            all_text(&call).contains(&expected_tagged),
            "envelope text block carries the tagged content"
        );
    }

    /// Round-trip: an anchor taken from a prior edit's `tagged_content` resolves
    /// against the on-disk file in an immediately-following `edit files` call,
    /// with NO intervening read.
    #[tokio::test]
    async fn anchor_from_prior_envelope_resolves_in_next_edit() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("roundtrip.txt");
        fs::write(&test_file, "alpha\nbeta\ngamma\n").unwrap();

        let context = crate::test_utils::create_test_context().await;

        // First edit changes line 2.
        let args = create_edit_arguments(&test_file.to_string_lossy(), "beta", "BETA", None);
        let call = execute_edit(args, &context).await.unwrap();
        let structured = call.structured_content.expect("structured content");
        let tagged = structured["mutation"]["tagged_content"]
            .as_str()
            .unwrap()
            .to_string();

        // Pull the `N:HH` anchor for the third line (gamma) straight from the
        // returned tagged_content — no intervening read.
        let anchor = tagged
            .lines()
            .find(|l| l.contains("|gamma"))
            .and_then(|l| l.split('|').next())
            .expect("gamma line present in tagged_content")
            .to_string();
        assert!(
            anchor.starts_with("3:"),
            "anchor should target line 3: {anchor}"
        );

        // Use that anchor as the `find` in a chained edit — it must resolve.
        let mut args2 = serde_json::Map::new();
        args2.insert(
            "file_path".to_string(),
            serde_json::Value::String(test_file.to_string_lossy().to_string()),
        );
        args2.insert("find".to_string(), serde_json::Value::String(anchor));
        args2.insert(
            "replace".to_string(),
            serde_json::Value::String("GAMMA".to_string()),
        );

        let call2 = execute_edit(args2, &context).await.unwrap();
        assert_eq!(
            call2.is_error,
            Some(false),
            "anchor must resolve: {call2:?}"
        );
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "alpha\nBETA\nGAMMA\n"
        );
    }

    /// An ambiguity result (no mutation) does NOT carry the envelope.
    #[tokio::test]
    async fn ambiguous_result_has_no_mutation_envelope() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("ambig_no_env.txt");
        let content = "head\nfoo()\nmid\nfoo()\ntail\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = ambiguity_args(&test_file.to_string_lossy(), "  foo()  ", "bar()", None);

        let call = execute_edit(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));
        // No structured envelope — nothing mutated.
        assert!(
            call.structured_content.is_none(),
            "ambiguity result carries no mutation envelope"
        );
        // File untouched.
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }

    /// A near-miss result (no mutation) does NOT carry the envelope.
    #[tokio::test]
    async fn near_miss_result_has_no_mutation_envelope() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("near_miss_no_env.txt");
        let content = "the quick brown fox\n";
        fs::write(&test_file, content).unwrap();

        let context = crate::test_utils::create_test_context().await;
        let args = create_edit_arguments(
            &test_file.to_string_lossy(),
            "the quick brown cat",
            "replacement",
            None,
        );

        let call = execute_edit(args, &context).await.unwrap();
        assert_eq!(call.is_error, Some(false));
        assert!(
            call.structured_content.is_none(),
            "near-miss result carries no mutation envelope"
        );
        assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
    }
}
