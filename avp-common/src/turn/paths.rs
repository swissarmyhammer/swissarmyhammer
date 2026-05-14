//! Path extraction utilities for detecting file paths in JSON values.

use std::path::{Path, PathBuf};

/// Known file-modifying tool names.
///
/// For these tools, we only scan top-level string values to avoid
/// picking up content strings like `old_string`, `new_string`, or `content`.
///
/// Single source of truth — exposed via [`is_known_file_tool`] so other
/// modules (like the file_tracker chain link) can answer "is this tool one
/// of the file-modifying tools whose Pre/Post must accumulate to turn-state?"
/// without maintaining a parallel list.
const KNOWN_FILE_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// Whether the given tool is one of the known file-modifying tools.
///
/// Returns true for tools whose Pre/PostToolUse hooks must accumulate file
/// changes into turn-state for Stop validators to consume. This is the
/// authoritative answer — both the path extractor in this module and the
/// file_tracker chain link's silent-failure diagnostics consult it.
pub fn is_known_file_tool(tool_name: &str) -> bool {
    KNOWN_FILE_TOOLS.contains(&tool_name)
}

/// Extract file paths from tool input using tool-aware strategy.
///
/// For known file-modifying tools (Edit, Write, MultiEdit, NotebookEdit):
/// - Only scans **top-level** string values (avoids content fields)
/// - Validates each candidate against the filesystem
///
/// For unknown tools:
/// - Falls back to recursive scanning with filesystem validation
///
/// All candidates must pass `is_path_structural()` and filesystem checks:
/// the file must exist, or its parent directory must exist (for new files).
pub fn extract_tool_paths(tool_name: &str, tool_input: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if KNOWN_FILE_TOOLS.contains(&tool_name) {
        extract_top_level_paths(tool_input, &mut paths);
    } else {
        extract_validated_recursive(tool_input, &mut paths);
    }

    paths
}

/// Extract validated paths from top-level string values of a JSON object only.
///
/// Used for known file-modifying tools where content fields should not be scanned.
fn extract_top_level_paths(value: &serde_json::Value, paths: &mut Vec<PathBuf>) {
    let serde_json::Value::Object(obj) = value else {
        return;
    };
    for val in obj.values() {
        let serde_json::Value::String(s) = val else {
            continue;
        };
        if let Some(path) = validate_path_candidate(s) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
}

/// Check if a string looks like a path AND exists on the filesystem.
///
/// Returns `Some(PathBuf)` if the whole string is path-like and either the file
/// exists or its parent directory exists (covering Write-new-file).
fn validate_path_candidate(s: &str) -> Option<PathBuf> {
    if !is_path_structural(s) {
        return None;
    }
    let path = Path::new(s);
    if path.exists() || path.parent().is_some_and(|p| p.exists()) {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Check if a string looks like a filesystem path using structural rules only.
///
/// Requires the whole string to look like a path by itself, rejecting multiline
/// strings (which are likely file content) and URL schemes. Does **not** consult
/// the filesystem.
///
/// Use this as a conservative syntactic gate before doing an expensive filesystem
/// check.
fn is_path_structural(s: &str) -> bool {
    if s.is_empty() || s.contains('\n') || s.contains("://") {
        return false;
    }

    // Absolute Unix
    if s.starts_with('/') && s.len() > 1 {
        return true;
    }

    // Relative
    if s.starts_with("./") || s.starts_with("../") {
        return true;
    }

    // Windows absolute
    if s.len() >= 3 {
        let bytes = s.as_bytes();
        if bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/')
        {
            return true;
        }
    }

    false
}

/// Recursively extract filesystem-validated paths from a JSON value.
fn extract_validated_recursive(value: &serde_json::Value, paths: &mut Vec<PathBuf>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(path) = validate_path_candidate(s) {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_validated_recursive(item, paths);
            }
        }
        serde_json::Value::Object(obj) => {
            for val in obj.values() {
                extract_validated_recursive(val, paths);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- extract_tool_paths tests ---

    #[test]
    fn test_extract_tool_paths_edit_skips_content() {
        // Create a real temp file so filesystem validation passes
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file_path = temp_dir.path().join("main.rs");
        std::fs::write(&file_path, "fn old() {}").unwrap();

        let value = json!({
            "file_path": file_path.to_string_lossy(),
            "old_string": "fn old() {}",
            "new_string": "fn new() {}"
        });

        let paths = extract_tool_paths("Edit", &value);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], file_path);
    }

    #[test]
    fn test_extract_tool_paths_write_new_file() {
        // Parent exists but file doesn't yet
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.rs");

        let value = json!({
            "file_path": file_path.to_string_lossy(),
            "content": "fn main() {}\nmore code\n"
        });

        let paths = extract_tool_paths("Write", &value);
        // Should find the path even though file doesn't exist (parent does)
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], file_path);
    }

    #[test]
    fn test_extract_tool_paths_write_ignores_multiline_content() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.rs");
        std::fs::write(&file_path, "").unwrap();

        let value = json!({
            "file_path": file_path.to_string_lossy(),
            "content": "line1\nline2\nline3"
        });

        let paths = extract_tool_paths("Write", &value);
        // Only file_path, not content (content has newlines, fails is_path_structural)
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], file_path);
    }

    #[test]
    fn test_extract_tool_paths_unknown_tool_recursive() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let file_path = temp_dir.path().join("data.txt");
        std::fs::write(&file_path, "data").unwrap();

        let value = json!({
            "nested": {
                "deep": {
                    "path": file_path.to_string_lossy()
                }
            }
        });

        // Unknown tool should scan recursively
        let paths = extract_tool_paths("CustomTool", &value);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], file_path);
    }

    #[test]
    fn test_extract_tool_paths_nonexistent_path_skipped() {
        let value = json!({
            "file_path": "/nonexistent/parent/dir/file.rs"
        });

        // Path doesn't exist and parent doesn't exist
        let paths = extract_tool_paths("Edit", &value);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_is_known_file_tool() {
        // The four canonical file-modifying tools.
        assert!(is_known_file_tool("Edit"));
        assert!(is_known_file_tool("Write"));
        assert!(is_known_file_tool("MultiEdit"));
        assert!(is_known_file_tool("NotebookEdit"));
        // Common tools that are not file-modifying.
        assert!(!is_known_file_tool("Read"));
        assert!(!is_known_file_tool("Bash"));
        assert!(!is_known_file_tool("Grep"));
        assert!(!is_known_file_tool("mcp__shell"));
        // Case sensitivity — tool names from Claude Code are exact.
        assert!(!is_known_file_tool("write"));
        assert!(!is_known_file_tool("WRITE"));
        // Empty string.
        assert!(!is_known_file_tool(""));
    }

    #[test]
    fn test_is_path_structural() {
        assert!(is_path_structural("/usr/bin/test"));
        assert!(is_path_structural("./relative"));
        assert!(is_path_structural("../parent"));
        assert!(!is_path_structural(""));
        assert!(!is_path_structural("just text"));
        assert!(!is_path_structural("/"));
        assert!(!is_path_structural("line1\nline2"));
        assert!(!is_path_structural("https://example.com"));
    }
}
