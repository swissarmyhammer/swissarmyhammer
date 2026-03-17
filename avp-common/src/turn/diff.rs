//! Unified diff computation for file changes.

use serde::{Deserialize, Serialize};
use similar::TextDiff;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Represents a diff for a single file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDiff {
    /// Path to the file that changed.
    pub path: PathBuf,
    /// Unified diff text (empty for binary files).
    pub diff_text: String,
    /// Whether this is a newly created file.
    pub is_new_file: bool,
    /// Whether the file is binary (no meaningful text diff).
    pub is_binary: bool,
}

/// Check if content appears to be binary (contains null bytes).
fn is_binary_content(content: &[u8]) -> bool {
    content.contains(&0)
}

/// Compute a unified diff between old and new content for a file.
///
/// - If `old_content` is `None`, treats the file as new (diff from empty).
/// - If content appears binary, returns a `FileDiff` with `is_binary: true` and empty diff text.
pub fn compute_diff(path: &Path, old_content: Option<&[u8]>, new_content: &[u8]) -> FileDiff {
    let is_new_file = old_content.is_none();
    let old_bytes = old_content.unwrap_or(b"");

    // Binary detection
    if is_binary_content(old_bytes) || is_binary_content(new_content) {
        return FileDiff {
            path: path.to_path_buf(),
            diff_text: String::new(),
            is_new_file,
            is_binary: true,
        };
    }

    let old_str = String::from_utf8_lossy(old_bytes);
    let new_str = String::from_utf8_lossy(new_content);

    let diff = TextDiff::from_lines(old_str.as_ref(), new_str.as_ref());
    let mut diff_text = String::new();

    let path_display = path.display().to_string();
    let _ = writeln!(
        diff_text,
        "--- {}",
        if is_new_file {
            "/dev/null"
        } else {
            &path_display
        }
    );
    let _ = writeln!(diff_text, "+++ {}", path_display);

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        let _ = write!(diff_text, "{}", hunk);
    }

    FileDiff {
        path: path.to_path_buf(),
        diff_text,
        is_new_file,
        is_binary: false,
    }
}

/// Format a list of diffs with fenced code blocks for display.
pub fn format_diffs_fenced(diffs: &[FileDiff]) -> String {
    let mut output = String::new();
    for diff in diffs {
        if diff.is_binary {
            let _ = writeln!(output, "Binary file changed: {}", diff.path.display());
        } else if !diff.diff_text.is_empty() {
            let _ = writeln!(output, "```diff");
            let _ = write!(output, "{}", diff.diff_text);
            let _ = writeln!(output, "```");
        }
    }
    output
}

/// Tools whose results should be rendered as diffs rather than YAML.
const DIFF_TOOLS: &[&str] = &["Edit", "Write", "NotebookEdit"];

/// Key used to embed diff text into the JSON value for rendering.
pub const DIFF_TEXT_KEY: &str = "_diff_text";

/// Fields to strip from Edit/Write tool results (bloated content).
const STRIP_TOOL_RESULT_FIELDS: &[&str] = &[
    "originalFile",
    "oldString",
    "newString",
    "structuredPatch",
    "replaceAll",
    "userModified",
    "filePath",
];

/// Fields to strip from Edit/Write tool input (duplicated content).
const STRIP_TOOL_INPUT_FIELDS: &[&str] = &["old_string", "new_string", "replace_all"];

/// Prepare a hook context JSON value for validators.
///
/// For Edit/Write tools with diffs: strips bloated fields (originalFile, etc.)
/// and embeds the diff text as `_diff_text`. For other tools: passes through as-is.
///
/// The returned value is still a `serde_json::Value::Object` — the rendering
/// layer converts it to YAML and appends diff blocks.
pub fn prepare_validator_context(
    mut input: serde_json::Value,
    diffs: Option<&[FileDiff]>,
) -> serde_json::Value {
    let tool_name = input
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let has_diffs = diffs.is_some_and(|d| !d.is_empty());
    let is_diff_tool = DIFF_TOOLS.contains(&tool_name.as_str());

    if !(is_diff_tool && has_diffs) {
        return input;
    }

    strip_object_fields(&mut input, "tool_result", STRIP_TOOL_RESULT_FIELDS);
    strip_object_fields(&mut input, "tool_input", STRIP_TOOL_INPUT_FIELDS);

    // Embed diff text
    let diff_text = format_diffs_fenced(diffs.unwrap());
    if let Some(map) = input.as_object_mut() {
        map.insert(
            DIFF_TEXT_KEY.to_string(),
            serde_json::Value::String(diff_text),
        );
    }

    input
}

/// Remove specified fields from a nested object within a JSON value.
fn strip_object_fields(value: &mut serde_json::Value, key: &str, fields: &[&str]) {
    if let Some(obj) = value.get_mut(key).and_then(|v| v.as_object_mut()) {
        for field in fields {
            obj.remove(*field);
        }
    }
}

/// Remove the diff text key and empty tool_result from a value.
fn remove_diff_text_and_empty_results(value: &serde_json::Value) -> serde_json::Value {
    let mut obj = value.clone();
    if let Some(map) = obj.as_object_mut() {
        map.remove(DIFF_TEXT_KEY);
        if let Some(tr) = map.get("tool_result") {
            if tr.as_object().is_some_and(|o| o.is_empty()) {
                map.remove("tool_result");
            }
        }
    }
    obj
}

/// Render a hook context value as a formatted string for validator prompts.
///
/// Produces YAML (not JSON). If the value contains a `_diff_text` field
/// (set by `prepare_validator_context` for edit tools), it is extracted
/// and appended as fenced diff blocks after the YAML header.
pub fn render_hook_context(value: &serde_json::Value) -> String {
    let mut out = String::new();

    // Extract diff text if present (don't include it in the YAML)
    let diff_text = value
        .get(DIFF_TEXT_KEY)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Build a clean copy for YAML rendering
    let clean_value = if diff_text.is_some() {
        remove_diff_text_and_empty_results(value)
    } else {
        value.clone()
    };

    // Render as YAML
    let _ = writeln!(out, "```yaml");
    match serde_yaml_ng::to_string(&clean_value) {
        Ok(yaml) => out.push_str(&yaml),
        Err(_) => {
            let _ = write!(
                out,
                "{}",
                serde_json::to_string_pretty(&clean_value)
                    .unwrap_or_else(|_| clean_value.to_string())
            );
        }
    }
    let _ = writeln!(out, "```");

    // Append diff blocks if present
    if let Some(diff) = diff_text {
        let _ = writeln!(out);
        out.push_str(&diff);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_edit() {
        let old = b"line 1\nline 2\nline 3\n";
        let new = b"line 1\nline 2 modified\nline 3\n";
        let diff = compute_diff(Path::new("/test/file.rs"), Some(old), new);

        assert_eq!(diff.path, PathBuf::from("/test/file.rs"));
        assert!(!diff.is_new_file);
        assert!(!diff.is_binary);
        assert!(diff.diff_text.contains("-line 2"));
        assert!(diff.diff_text.contains("+line 2 modified"));
    }

    #[test]
    fn test_compute_diff_new_file() {
        let new = b"new content\n";
        let diff = compute_diff(Path::new("/test/new.rs"), None, new);

        assert!(diff.is_new_file);
        assert!(!diff.is_binary);
        assert!(diff.diff_text.contains("--- /dev/null"));
        assert!(diff.diff_text.contains("+new content"));
    }

    #[test]
    fn test_compute_diff_binary() {
        let old = b"text content";
        let new = b"binary \x00 content";
        let diff = compute_diff(Path::new("/test/file.bin"), Some(old), new);

        assert!(diff.is_binary);
        assert!(diff.diff_text.is_empty());
    }

    #[test]
    fn test_compute_diff_no_change() {
        let content = b"same content\n";
        let diff = compute_diff(Path::new("/test/file.rs"), Some(content), content);

        assert!(!diff.is_new_file);
        assert!(!diff.is_binary);
        // Diff text should have headers but no hunks
        assert!(diff.diff_text.contains("---"));
        assert!(!diff.diff_text.contains("@@"));
    }

    #[test]
    fn test_format_diffs_fenced() {
        let diffs = vec![
            FileDiff {
                path: PathBuf::from("/test/file.rs"),
                diff_text: "--- /test/file.rs\n+++ /test/file.rs\n@@ -1 +1 @@\n-old\n+new\n"
                    .to_string(),
                is_new_file: false,
                is_binary: false,
            },
            FileDiff {
                path: PathBuf::from("/test/image.png"),
                diff_text: String::new(),
                is_new_file: false,
                is_binary: true,
            },
        ];

        let output = format_diffs_fenced(&diffs);
        assert!(output.contains("```diff"));
        assert!(output.contains("```"));
        assert!(output.contains("Binary file changed: /test/image.png"));
    }

    #[test]
    fn test_binary_old_content() {
        let old = b"binary \x00 old";
        let new = b"text content";
        let diff = compute_diff(Path::new("/test/file"), Some(old), new);
        assert!(diff.is_binary);
    }

    /// End-to-end test: realistic Edit tool JSON → compute diff → prepare → render.
    /// This mirrors the exact data flow in production.
    #[test]
    fn test_e2e_edit_tool_with_real_diff() {
        // 1. Simulate real file contents (before and after an edit)
        let old_content = b"/// A scratch file.\nfn main() {\n    println!(\"hello\");\n}\n";
        let new_content = b"/// A scratch file.\nfn main() {\n    println!(\"hello world\");\n}\n";

        // 2. Compute a real diff (this is what PostToolUseFileTracker does)
        let diff = compute_diff(
            Path::new("/project/src/main.rs"),
            Some(old_content),
            new_content,
        );
        assert!(diff.diff_text.contains("-    println!(\"hello\");"));
        assert!(diff.diff_text.contains("+    println!(\"hello world\");"));

        // 3. Realistic PostToolUse JSON (what Claude Code sends)
        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "cwd": "/project",
            "session_id": "abc-123",
            "permission_mode": "bypassPermissions",
            "tool_name": "Edit",
            "tool_use_id": "toolu_01XYZ",
            "tool_input": {
                "file_path": "/project/src/main.rs",
                "old_string": "    println!(\"hello\");",
                "new_string": "    println!(\"hello world\");",
                "replace_all": false
            },
            "tool_result": {
                "filePath": "/project/src/main.rs",
                "oldString": "    println!(\"hello\");",
                "newString": "    println!(\"hello world\");",
                "originalFile": "/// A scratch file.\nfn main() {\n    println!(\"hello\");\n}\n",
                "replaceAll": false,
                "structuredPatch": [{"lines": ["-old", "+new"], "oldStart": 1, "newStart": 1, "oldLines": 1, "newLines": 1}],
                "userModified": false
            },
            "transcript_path": "/some/path.jsonl"
        });

        // 4. Prepare (strip bloated fields, embed diff text)
        let prepared = prepare_validator_context(input, Some(&[diff]));

        // 5. Render to string (what the validator LLM sees)
        let output = render_hook_context(&prepared);

        // Print for debugging
        eprintln!("=== RENDERED OUTPUT ===\n{}\n=== END ===", output);

        // ASSERTIONS: what the validator should see

        // Must have YAML block (not JSON)
        assert!(
            output.contains("```yaml"),
            "Should contain ```yaml fence, got:\n{}",
            output
        );
        assert!(
            !output.contains("```json"),
            "Should NOT contain ```json fence, got:\n{}",
            output
        );

        // Must have the tool metadata in YAML
        assert!(output.contains("tool_name: Edit"), "Missing tool_name");
        assert!(output.contains("cwd: /project"), "Missing cwd");

        // Must have diff block with actual diff lines
        assert!(
            output.contains("```diff"),
            "Should contain ```diff fence, got:\n{}",
            output
        );
        assert!(
            output.contains("-    println!(\"hello\");"),
            "Missing removed line"
        );
        assert!(
            output.contains("+    println!(\"hello world\");"),
            "Missing added line"
        );

        // Must NOT have the bloated fields from tool_result
        assert!(
            !output.contains("originalFile"),
            "Should not contain originalFile"
        );
        assert!(
            !output.contains("structuredPatch"),
            "Should not contain structuredPatch"
        );
        assert!(
            !output.contains("oldString"),
            "Should not contain oldString from tool_result"
        );
        assert!(
            !output.contains("newString"),
            "Should not contain newString from tool_result"
        );
    }

    /// End-to-end test: Bash tool JSON → render as YAML (no diffs).
    #[test]
    fn test_e2e_bash_tool_as_yaml() {
        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "cwd": "/project",
            "session_id": "abc-123",
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo test"
            },
            "tool_result": {
                "stdout": "test result: ok. 42 passed",
                "exit_code": 0
            }
        });

        // No diffs for Bash
        let prepared = prepare_validator_context(input, None);
        let output = render_hook_context(&prepared);

        eprintln!("=== BASH OUTPUT ===\n{}\n=== END ===", output);

        // Must be YAML, not JSON
        assert!(output.contains("```yaml"), "Should be ```yaml");
        assert!(!output.contains("```json"), "Should NOT be ```json");
        assert!(output.contains("tool_name: Bash"), "Missing tool_name");
        assert!(output.contains("command: cargo test"), "Missing command");

        // Must NOT contain JSON syntax
        assert!(
            !output.contains("\"tool_name\""),
            "Should not contain JSON-quoted keys"
        );
    }

    /// Test: Edit without diffs available falls back to YAML (no diff block).
    #[test]
    fn test_e2e_edit_without_diffs() {
        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": "/project/src/main.rs",
                "old_string": "old",
                "new_string": "new"
            },
            "cwd": "/project"
        });

        // No diffs available
        let prepared = prepare_validator_context(input, None);
        let output = render_hook_context(&prepared);

        // Should render as YAML with all fields intact (no stripping without diffs)
        assert!(output.contains("```yaml"));
        assert!(output.contains("tool_name: Edit"));
        assert!(output.contains("old_string: old"));
        assert!(!output.contains("```diff"), "No diff block without diffs");
    }
}
