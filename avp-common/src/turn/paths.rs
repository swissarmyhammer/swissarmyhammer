//! Path extraction utilities for detecting file paths in JSON values.

use std::path::PathBuf;

/// Extract file paths from a JSON value by recursively scanning all string values.
///
/// This function looks for strings that appear to be file paths:
/// - Absolute paths starting with `/`
/// - Relative paths starting with `./` or `../`
///
/// It recursively scans objects and arrays to find all string values.
pub fn extract_paths(value: &serde_json::Value) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    extract_paths_recursive(value, &mut paths);
    paths
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
}
