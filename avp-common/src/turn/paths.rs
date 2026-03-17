//! Path extraction utilities for detecting file paths in JSON values.

use std::path::{Path, PathBuf};

/// Known file-modifying tool names.
///
/// For these tools, we only scan top-level string values to avoid
/// picking up content strings like `old_string`, `new_string`, or `content`.
const KNOWN_FILE_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// Extract file paths from tool input using tool-aware strategy.
///
/// For known file-modifying tools (Edit, Write, MultiEdit, NotebookEdit):
/// - Only scans **top-level** string values (avoids content fields)
/// - Validates each candidate against the filesystem
///
/// For unknown tools:
/// - Falls back to recursive scanning with filesystem validation
///
/// All candidates must pass `is_path_like()` and filesystem checks:
/// the file must exist, or its parent directory must exist (for new files).
pub fn extract_tool_paths(tool_name: &str, tool_input: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if KNOWN_FILE_TOOLS.contains(&tool_name) {
        // Top-level scan only for known tools
        if let serde_json::Value::Object(obj) = tool_input {
            for val in obj.values() {
                if let serde_json::Value::String(s) = val {
                    if let Some(path) = validate_path_candidate(s) {
                        if !paths.contains(&path) {
                            paths.push(path);
                        }
                    }
                }
            }
        }
    } else {
        // Recursive scan for unknown tools
        extract_validated_recursive(tool_input, &mut paths);
    }

    paths
}

/// Extract file paths from a JSON value by recursively scanning all string values.
///
/// This is the original extraction function kept for backward compatibility.
/// Prefer `extract_tool_paths()` for tool-aware extraction with filesystem validation.
pub fn extract_paths(value: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    extract_paths_recursive(value, &mut paths);
    paths
}

/// Check if a string looks like a path AND exists on the filesystem.
///
/// Returns `Some(PathBuf)` if the whole string is path-like and either the file
/// exists or its parent directory exists (covering Write-new-file).
fn validate_path_candidate(s: &str) -> Option<PathBuf> {
    if !is_path_like(s) {
        return None;
    }
    let path = Path::new(s);
    if path.exists() || path.parent().is_some_and(|p| p.exists()) {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Check if a string looks like a filesystem path (structural check only).
///
/// This is stricter than `is_likely_path` — it requires the whole string to be
/// a path-like value, filtering out multiline content and URLs.
fn is_path_like(s: &str) -> bool {
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

fn extract_paths_recursive(value: &serde_json::Value, paths: &mut Vec<PathBuf>) {
    match value {
        serde_json::Value::String(s) => {
            if is_likely_path(s) {
                let path = PathBuf::from(s);
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_paths_recursive(item, paths);
            }
        }
        serde_json::Value::Object(obj) => {
            for (_key, val) in obj {
                extract_paths_recursive(val, paths);
            }
        }
        _ => {}
    }
}

/// Determine if a string is likely a file path.
///
/// This uses heuristics to identify path-like strings:
/// - Starts with `/` (absolute Unix path)
/// - Starts with `./` or `../` (relative path)
/// - Starts with a drive letter like `C:\` (Windows absolute path)
fn is_likely_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // Absolute Unix paths
    if s.starts_with('/') {
        // Filter out things that are clearly not file paths:
        // - URLs: contain "://"
        // - Comments: "// " (two slashes followed by space)
        // - Too short to be useful
        // - Contains newlines (likely multi-line content)
        if s.contains("://") || s.starts_with("// ") || s.len() <= 1 || s.contains('\n') {
            return false;
        }
        return true;
    }

    // Relative paths
    if s.starts_with("./") || s.starts_with("../") {
        // Also filter out multi-line content
        return !s.contains('\n');
    }

    // Windows absolute paths (C:\, D:\, etc.)
    if s.len() >= 3 {
        let chars: Vec<char> = s.chars().take(3).collect();
        if chars.len() == 3
            && chars[0].is_ascii_alphabetic()
            && chars[1] == ':'
            && (chars[2] == '\\' || chars[2] == '/')
        {
            // Filter out multi-line content
            return !s.contains('\n');
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_paths_simple_object() {
        let value = json!({
            "file_path": "/path/to/file.rs",
            "other": "not a path"
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/path/to/file.rs"));
    }

    #[test]
    fn test_extract_paths_nested_object() {
        let value = json!({
            "outer": {
                "inner": {
                    "path": "/deeply/nested/file.txt"
                }
            }
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/deeply/nested/file.txt"));
    }

    #[test]
    fn test_extract_paths_array() {
        let value = json!({
            "files": [
                "/path/one.rs",
                "/path/two.rs",
                "not a path"
            ]
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("/path/one.rs")));
        assert!(paths.contains(&PathBuf::from("/path/two.rs")));
    }

    #[test]
    fn test_extract_paths_relative() {
        let value = json!({
            "path1": "./relative/path.rs",
            "path2": "../parent/path.rs"
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("./relative/path.rs")));
        assert!(paths.contains(&PathBuf::from("../parent/path.rs")));
    }

    #[test]
    fn test_extract_paths_windows() {
        let value = json!({
            "path": "C:\\Users\\test\\file.rs"
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("C:\\Users\\test\\file.rs"));
    }

    #[test]
    fn test_extract_paths_no_duplicates() {
        let value = json!({
            "path1": "/same/path.rs",
            "path2": "/same/path.rs"
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn test_extract_paths_mixed_content() {
        let value = json!({
            "file_path": "/path/to/file.rs",
            "content": "some text content",
            "number": 42,
            "boolean": true,
            "null_val": null,
            "nested": {
                "another_path": "/another/path.txt"
            }
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_extract_paths_empty_string() {
        let value = json!({
            "path": ""
        });

        let paths = extract_paths(&value);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_extract_paths_url_not_path() {
        let value = json!({
            "url": "https://example.com/path"
        });

        let paths = extract_paths(&value);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_is_likely_path() {
        // Absolute Unix paths
        assert!(is_likely_path("/path/to/file.rs"));
        assert!(is_likely_path("/a"));

        // Relative paths
        assert!(is_likely_path("./path/to/file.rs"));
        assert!(is_likely_path("../path/to/file.rs"));

        // Windows paths
        assert!(is_likely_path("C:\\path\\to\\file.rs"));
        assert!(is_likely_path("D:/path/to/file.rs"));

        // Not paths
        assert!(!is_likely_path(""));
        assert!(!is_likely_path("just some text"));
        assert!(!is_likely_path("https://example.com"));
        assert!(!is_likely_path("/")); // Root alone is not useful
    }

    #[test]
    fn test_extract_paths_edit_tool_input() {
        // Simulates actual Edit tool input structure
        let value = json!({
            "file_path": "/Users/test/project/src/main.rs",
            "old_string": "fn old() {}",
            "new_string": "fn new() {}"
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/Users/test/project/src/main.rs"));
    }

    #[test]
    fn test_extract_paths_write_tool_input() {
        // Simulates actual Write tool input structure
        let value = json!({
            "file_path": "/Users/test/project/src/new_file.rs",
            "content": "// New file content\nfn main() {}"
        });

        let paths = extract_paths(&value);
        assert_eq!(paths.len(), 1);
        assert_eq!(
            paths[0],
            PathBuf::from("/Users/test/project/src/new_file.rs")
        );
    }

    #[test]
    fn test_extract_paths_bash_with_paths() {
        // Simulates Bash tool input that happens to contain paths
        let value = json!({
            "command": "cat /etc/passwd",
            "working_dir": "/home/user/project"
        });

        let paths = extract_paths(&value);
        // Should find /etc/passwd in the command string and /home/user/project
        // Actually, "cat /etc/passwd" is a single string, not a path
        // Only /home/user/project should be detected
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], PathBuf::from("/home/user/project"));
    }

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
        // Only file_path, not content (content has newlines, fails is_path_like)
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
    fn test_is_path_like() {
        assert!(is_path_like("/usr/bin/test"));
        assert!(is_path_like("./relative"));
        assert!(is_path_like("../parent"));
        assert!(!is_path_like(""));
        assert!(!is_path_like("just text"));
        assert!(!is_path_like("/"));
        assert!(!is_path_like("line1\nline2"));
        assert!(!is_path_like("https://example.com"));
    }
}
