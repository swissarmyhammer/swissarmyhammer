//! Unified diff computation for file changes.

use serde::{Deserialize, Serialize};
use similar::TextDiff;
use std::fmt::Write;
use std::path::{Path, PathBuf};

/// Represents a diff for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}
