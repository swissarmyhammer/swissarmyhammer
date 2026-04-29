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
///
/// `content` is the full post-edit file body — once `_diff_text` is embedded,
/// this is redundant and would 2-3× the prompt size. `originalFile` is the
/// pre-edit body, also already represented as `-` lines in the diff.
const STRIP_TOOL_RESULT_FIELDS: &[&str] = &[
    "content",
    "originalFile",
    "oldString",
    "newString",
    "structuredPatch",
    "replaceAll",
    "userModified",
    "filePath",
];

/// Fields to strip from Edit/Write tool input (duplicated content).
///
/// `content` is the full file body that Write sends in `tool_input` — once
/// `_diff_text` is embedded, this is redundant. `old_string`/`new_string` are
/// already represented as `-`/`+` lines in the diff.
const STRIP_TOOL_INPUT_FIELDS: &[&str] = &["content", "old_string", "new_string", "replace_all"];

/// Prepare a hook context JSON value for validators.
///
/// Whenever the caller supplies non-empty `diffs`, the rendered diff text is
/// embedded into the input as `_diff_text` so the renderer appends it as a
/// fenced block after the YAML payload. This is universal — Stop hooks (which
/// have no `tool_name`), PostToolUse Edit/Write hooks, and any other caller
/// that prepared diffs upstream all benefit.
///
/// In addition, when the input is for an Edit/Write tool (i.e. `tool_name`
/// is one of [`DIFF_TOOLS`]), the bloated `tool_input` / `tool_result` fields
/// that duplicate the diff content are stripped. Stripping is conditional on
/// the edit-tool shape because the field names being stripped are specific
/// to those tool payloads; embedding the diff itself is not.
///
/// The returned value is still a `serde_json::Value::Object` — the rendering
/// layer converts it to YAML and appends diff blocks.
pub fn prepare_validator_context(
    mut input: serde_json::Value,
    diffs: Option<&[FileDiff]>,
) -> serde_json::Value {
    // Caller didn't supply diffs (or supplied an empty slice): nothing to embed.
    let Some(diffs) = diffs.filter(|d| !d.is_empty()) else {
        return input;
    };

    // Edit/Write payloads carry duplicated old/new content alongside the diff;
    // strip those so the prompt isn't padded with redundant text. Stop-hook
    // and other inputs don't have those fields and don't need stripping.
    let tool_name = input
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if DIFF_TOOLS.contains(&tool_name.as_str()) {
        strip_object_fields(&mut input, "tool_result", STRIP_TOOL_RESULT_FIELDS);
        strip_object_fields(&mut input, "tool_input", STRIP_TOOL_INPUT_FIELDS);
    }

    // Embed diff text — universal across all callers that prepared diffs.
    let diff_text = format_diffs_fenced(diffs);
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

    /// Stop hook context with filtered diffs: diffs are embedded as `_diff_text`
    /// even though there is no `tool_name` (Stop is a turn-end event, not a tool
    /// event). The caller already filtered to .rs files; the renderer appends
    /// only those.
    #[test]
    fn test_stop_hook_filtered_diffs_renders_only_matching() {
        // Simulate a Stop hook with mixed file type diffs (a.rs, b.py, c.rs)
        let rs_diffs = vec![
            FileDiff {
                path: PathBuf::from("a.rs"),
                diff_text: "--- a.rs\n+++ a.rs\n@@ -1 +1 @@\n-old_a\n+new_a\n".to_string(),
                is_new_file: false,
                is_binary: false,
            },
            FileDiff {
                path: PathBuf::from("c.rs"),
                diff_text: "--- c.rs\n+++ c.rs\n@@ -1 +1 @@\n-old_c\n+new_c\n".to_string(),
                is_new_file: false,
                is_binary: false,
            },
        ];

        // Stop hook input (no tool_name) — caller-supplied diffs MUST still be
        // embedded so the validator sees them.
        let input = serde_json::json!({
            "hook_event_name": "Stop",
            "cwd": "/project",
            "session_id": "test-session"
        });

        let prepared = prepare_validator_context(input, Some(&rs_diffs));
        let output = render_hook_context(&prepared);

        assert!(output.contains("```yaml"));
        assert!(output.contains("hook_event_name: Stop"));

        // Diff content must be present in the rendered output.
        assert!(
            output.contains("```diff"),
            "Stop hook should render a diff block"
        );
        assert!(output.contains("old_a"), "a.rs diff content should appear");
        assert!(output.contains("old_c"), "c.rs diff content should appear");
        assert!(
            !output.contains("old_b"),
            "b.py was filtered out by the caller"
        );
    }

    /// Stop hook context with no diffs: no diff blocks in output.
    #[test]
    fn test_stop_hook_no_diffs() {
        let input = serde_json::json!({
            "hook_event_name": "Stop",
            "cwd": "/project",
            "session_id": "test-session"
        });

        let prepared = prepare_validator_context(input, None);
        let output = render_hook_context(&prepared);

        assert!(output.contains("```yaml"));
        assert!(output.contains("hook_event_name: Stop"));
        assert!(!output.contains("```diff"), "No diff block without diffs");
    }

    /// Stop-hook style input (no `tool_name`) plus non-empty diffs MUST embed
    /// `_diff_text` containing the diff content. This is a regression test for
    /// the bug where `prepare_validator_context` short-circuited on Stop hooks
    /// because `tool_name` wasn't in `DIFF_TOOLS`, dropping the caller-supplied
    /// diffs on the floor.
    #[test]
    fn test_prepare_validator_context_stop_hook_embeds_diff_text() {
        let diffs = vec![FileDiff {
            path: PathBuf::from("src/lib.rs"),
            diff_text: "--- src/lib.rs\n+++ src/lib.rs\n@@ -1 +1 @@\n-old_line\n+new_line\n"
                .to_string(),
            is_new_file: false,
            is_binary: false,
        }];

        // Stop-hook input has no tool_name field at all.
        let input = serde_json::json!({
            "hook_event_name": "Stop",
            "cwd": "/project",
            "session_id": "abc-123"
        });

        let prepared = prepare_validator_context(input, Some(&diffs));

        // `_diff_text` must be present and contain the diff content.
        let diff_text = prepared
            .get(DIFF_TEXT_KEY)
            .and_then(|v| v.as_str())
            .expect("_diff_text should be embedded for Stop-hook inputs with diffs");
        assert!(
            diff_text.contains("-old_line"),
            "_diff_text should contain removed line, got: {}",
            diff_text
        );
        assert!(
            diff_text.contains("+new_line"),
            "_diff_text should contain added line, got: {}",
            diff_text
        );
        assert!(
            diff_text.contains("```diff"),
            "_diff_text should be a fenced diff block, got: {}",
            diff_text
        );
    }

    /// All diffs pass through when no file patterns filter them.
    #[test]
    fn test_stop_hook_all_diffs_without_filtering() {
        let all_diffs = vec![
            FileDiff {
                path: PathBuf::from("a.rs"),
                diff_text: "--- a.rs\n+++ a.rs\n@@ -1 +1 @@\n-old_a\n+new_a\n".to_string(),
                is_new_file: false,
                is_binary: false,
            },
            FileDiff {
                path: PathBuf::from("b.py"),
                diff_text: "--- b.py\n+++ b.py\n@@ -1 +1 @@\n-old_b\n+new_b\n".to_string(),
                is_new_file: false,
                is_binary: false,
            },
            FileDiff {
                path: PathBuf::from("c.rs"),
                diff_text: "--- c.rs\n+++ c.rs\n@@ -1 +1 @@\n-old_c\n+new_c\n".to_string(),
                is_new_file: false,
                is_binary: false,
            },
        ];

        // format_diffs_fenced with all diffs (no filtering)
        let diff_output = format_diffs_fenced(&all_diffs);
        assert!(diff_output.contains("old_a"), "Should contain a.rs diff");
        assert!(diff_output.contains("old_b"), "Should contain b.py diff");
        assert!(diff_output.contains("old_c"), "Should contain c.rs diff");
    }

    /// Strip behavior: when an Edit-style PostToolUse input carries a full
    /// `tool_input.content`, a full `tool_result.content`, and a full
    /// `tool_result.originalFile`, each duplicating the file body, the
    /// rendered output must contain exactly one copy of the file body
    /// (inside the diff block) and zero copies of the bloat fields.
    ///
    /// Regression test for kanban 01KQ8CZG7M0S00BV2C77T7QZV2 — PostToolUse
    /// validator prompts duplicated file content 3× because `content` was
    /// missing from `STRIP_TOOL_INPUT_FIELDS` / `STRIP_TOOL_RESULT_FIELDS`.
    #[test]
    fn test_strip_removes_duplicated_content_fields() {
        // A unique line that only appears in the file body (and therefore
        // only legitimately appears once, inside the diff block).
        let unique_marker = "UNIQUE_FILE_CONTENT_MARKER_42";
        let body = format!(
            "use std::time::Duration;\nfn main() {{\n    let _ = \"{}\";\n}}\n",
            unique_marker
        );

        // Real diff for an Edit that changed one line of the body.
        let old_body = body.replace(unique_marker, "OLD_MARKER");
        let diff = compute_diff(
            Path::new("/project/src/sample.rs"),
            Some(old_body.as_bytes()),
            body.as_bytes(),
        );

        // PostToolUse Edit hook with all three duplicated content fields.
        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "cwd": "/project",
            "session_id": "test",
            "tool_name": "Edit",
            "tool_use_id": "toolu_test",
            "tool_input": {
                "file_path": "/project/src/sample.rs",
                "old_string": "OLD_MARKER",
                "new_string": unique_marker,
                "content": body,
            },
            "tool_result": {
                "filePath": "/project/src/sample.rs",
                "content": body,
                "originalFile": old_body,
                "oldString": "OLD_MARKER",
                "newString": unique_marker,
                "structuredPatch": [],
                "replaceAll": false,
                "userModified": false
            }
        });

        let prepared = prepare_validator_context(input, Some(&[diff]));
        let output = render_hook_context(&prepared);

        // The unique marker must appear exactly once: in the `+` diff line.
        let marker_count = output.matches(unique_marker).count();
        assert_eq!(
            marker_count, 1,
            "file content marker should appear exactly once (in diff), \
             found {} occurrences in:\n{}",
            marker_count, output
        );

        // None of the bloat field names should appear in the rendered YAML.
        for field in [
            "originalFile",
            "structuredPatch",
            "oldString",
            "newString",
            "replaceAll",
            "userModified",
        ] {
            assert!(
                !output.contains(field),
                "bloat field `{}` should be stripped, but appeared in:\n{}",
                field,
                output
            );
        }
        // `content:` (YAML key) must not appear — it was stripped from both
        // tool_input and tool_result. Use the YAML key form so we don't match
        // `content` as a substring of unrelated words.
        assert!(
            !output.contains("content:"),
            "`content:` key should be stripped from tool_input and tool_result, \
             got:\n{}",
            output
        );

        // The diff itself must still be present.
        assert!(
            output.contains("```diff"),
            "diff fence missing:\n{}",
            output
        );
        assert!(
            output.contains(&format!("+    let _ = \"{}\";", unique_marker)),
            "added line missing from diff:\n{}",
            output
        );
    }

    /// Bounded prompt size: a Write of a 70-line file produces a rendered
    /// prompt no larger than ~1.5× the diff size. Before the fix the prompt
    /// was ~3× because `tool_input.content` and `tool_result.content` each
    /// repeated the full file body.
    ///
    /// Snapshot-style assertion: not an exact byte count (which would be
    /// brittle to formatting changes), but a hard upper bound that catches
    /// the duplication regression.
    #[test]
    fn test_rendered_prompt_size_bounded_by_diff_size() {
        // Realistic ~70-line file body.
        let mut body = String::from("use std::time::Duration;\n\npub struct RetryClient {\n    timeout: Duration,\n}\n\nimpl RetryClient {\n    pub fn new(timeout: Duration) -> Self {\n        Self { timeout }\n    }\n\n");
        for i in 0..50 {
            body.push_str(&format!(
                "    pub fn method_{}(&self) -> u64 {{ {} }}\n",
                i, i
            ));
        }
        body.push_str("}\n");
        assert!(
            body.lines().count() >= 60,
            "test fixture should be at least 60 lines"
        );

        // Write of a brand-new file → diff is `--- /dev/null` + every line
        // as `+`.
        let diff = compute_diff(Path::new("/project/src/retry.rs"), None, body.as_bytes());
        let diff_size = diff.diff_text.len();

        // PostToolUse Write hook as Claude Code sends it: full body in BOTH
        // tool_input.content AND tool_result.content.
        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "cwd": "/project",
            "session_id": "test",
            "tool_name": "Write",
            "tool_use_id": "toolu_test",
            "tool_input": {
                "file_path": "/project/src/retry.rs",
                "content": body,
            },
            "tool_result": {
                "filePath": "/project/src/retry.rs",
                "content": body,
                "type": "create"
            }
        });

        let prepared = prepare_validator_context(input, Some(&[diff]));
        let output = render_hook_context(&prepared);

        // Upper bound: prompt should be at most 1.5× the diff size + small
        // boilerplate (YAML keys, fences, hook metadata). Allow 1KB of slack
        // for formatting overhead independent of file size.
        let upper_bound = (diff_size as f64 * 1.5) as usize + 1024;
        assert!(
            output.len() <= upper_bound,
            "rendered prompt size {} exceeds bound {} (diff_size={}); \
             content may be duplicated:\n{}",
            output.len(),
            upper_bound,
            diff_size,
            output
        );
    }

    /// Token-count style assertion: render the same Write input with the
    /// strip lists artificially emptied (mimicking pre-fix behavior) and with
    /// the real strip lists. Assert the real (post-fix) output is at least
    /// 50% smaller — the duplicated body accounts for ~2/3 of the pre-fix
    /// prompt, so 50% is a comfortable lower bound that still catches a
    /// regression where stripping silently stops working.
    #[test]
    fn test_strip_reduces_prompt_size_by_at_least_50_percent() {
        // Build a realistic body large enough that duplication dominates.
        let mut body = String::new();
        for i in 0..100 {
            body.push_str(&format!(
                "    // line {} of a representative source file body\n",
                i
            ));
        }

        let diff = compute_diff(Path::new("/project/src/big.rs"), None, body.as_bytes());

        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "cwd": "/project",
            "session_id": "test",
            "tool_name": "Write",
            "tool_use_id": "toolu_test",
            "tool_input": {
                "file_path": "/project/src/big.rs",
                "content": body,
            },
            "tool_result": {
                "filePath": "/project/src/big.rs",
                "content": body,
            }
        });

        // Pre-fix simulation: render the input *without* preparing it.
        // `render_hook_context` with no `_diff_text` embedded just dumps the
        // YAML, including both `content` fields verbatim — exactly the
        // behavior the strip list is meant to prevent.
        let pre_fix_rendered = render_hook_context(&input);
        let pre_fix_size = pre_fix_rendered.len();

        // Post-fix: prepare (strips bloat, embeds diff) then render.
        let prepared = prepare_validator_context(input, Some(&[diff]));
        let post_fix_rendered = render_hook_context(&prepared);
        let post_fix_size = post_fix_rendered.len();

        let reduction_ratio = (pre_fix_size - post_fix_size) as f64 / pre_fix_size as f64;
        assert!(
            reduction_ratio >= 0.50,
            "strip should reduce prompt size by at least 50% \
             (pre={} bytes, post={} bytes, reduction={:.1}%)",
            pre_fix_size,
            post_fix_size,
            reduction_ratio * 100.0
        );
    }
}
