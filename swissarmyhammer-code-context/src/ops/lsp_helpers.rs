//! Shared LSP protocol helpers used across op modules.
//!
//! Centralises functions that were previously duplicated in multiple op files:
//! URI conversion, language ID detection, source range reading, and LSP JSON
//! range parsing.

use crate::layered_context::LspRange;

/// Convert a file path to a `file://` URI.
///
/// Absolute paths are prefixed directly. Relative paths are resolved against
/// the current working directory so that the resulting URI is always absolute,
/// which is what LSP servers require.
pub fn file_path_to_uri(path: &str) -> String {
    if path.starts_with('/') {
        format!("file://{}", path)
    } else {
        // Relative path -- resolve against cwd to produce an absolute URI.
        let abs = std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| std::path::PathBuf::from(path));
        format!("file://{}", abs.display())
    }
}

/// Convert a `file://` URI to a filesystem path.
///
/// Strips the `file://` prefix. Returns the URI as-is for non-file schemes.
pub fn uri_to_file_path(uri: &str) -> String {
    if let Some(path) = uri.strip_prefix("file://") {
        path.to_string()
    } else {
        uri.to_string()
    }
}

/// Guess an LSP `languageId` string from a file extension.
///
/// Returns `"plaintext"` for unrecognised or missing extensions.
pub fn language_id_from_path(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
    {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("jsx") => "javascriptreact",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp" | "cc" | "cxx") => "cpp",
        Some("h" | "hpp") => "cpp",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("cs") => "csharp",
        Some("swift") => "swift",
        Some("kt") => "kotlin",
        Some("lua") => "lua",
        Some("zig") => "zig",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("sh" | "bash") => "shellscript",
        _ => "plaintext",
    }
}

/// Read source lines from a file on disk for the given LSP range.
///
/// Returns `None` if the file cannot be read or the range is out of bounds.
pub fn read_source_range(file_path: &str, range: &LspRange) -> Option<String> {
    let content = std::fs::read_to_string(file_path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    let start = range.start_line as usize;
    let end = (range.end_line as usize).min(lines.len().saturating_sub(1));

    if start > end || start >= lines.len() {
        return None;
    }

    let selected: Vec<&str> = lines[start..=end].to_vec();
    Some(selected.join("\n"))
}

/// Parse an LSP range JSON object into an [`LspRange`].
///
/// Expects the standard LSP format:
/// `{ "start": { "line": N, "character": N }, "end": { "line": N, "character": N } }`.
///
/// Returns `None` if any required field is missing or not a valid integer.
pub fn parse_lsp_range(range: &serde_json::Value) -> Option<LspRange> {
    let start = range.get("start")?;
    let end = range.get("end")?;

    Some(LspRange {
        start_line: start.get("line")?.as_u64()? as u32,
        start_character: start.get("character")?.as_u64()? as u32,
        end_line: end.get("line")?.as_u64()? as u32,
        end_character: end.get("character")?.as_u64()? as u32,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_path_to_uri_absolute() {
        let uri = file_path_to_uri("/home/user/project/src/main.rs");
        assert_eq!(uri, "file:///home/user/project/src/main.rs");
    }

    #[test]
    fn test_file_path_to_uri_relative_resolves_to_absolute() {
        let uri = file_path_to_uri("src/main.rs");
        assert!(
            uri.starts_with("file:///"),
            "URI should start with file:///: {}",
            uri
        );
        assert!(
            uri.ends_with("src/main.rs"),
            "URI should end with the relative path: {}",
            uri
        );
    }

    #[test]
    fn test_uri_to_file_path_strips_prefix() {
        assert_eq!(
            uri_to_file_path("file:///usr/src/main.rs"),
            "/usr/src/main.rs"
        );
    }

    #[test]
    fn test_uri_to_file_path_non_file_scheme() {
        assert_eq!(
            uri_to_file_path("https://example.com/file.rs"),
            "https://example.com/file.rs"
        );
    }

    #[test]
    fn test_uri_to_file_path_raw_path() {
        assert_eq!(uri_to_file_path("/raw/path.rs"), "/raw/path.rs");
    }

    #[test]
    fn test_language_id_known_extensions() {
        assert_eq!(language_id_from_path("main.rs"), "rust");
        assert_eq!(language_id_from_path("app.py"), "python");
        assert_eq!(language_id_from_path("index.ts"), "typescript");
        assert_eq!(language_id_from_path("component.tsx"), "typescriptreact");
        assert_eq!(language_id_from_path("main.go"), "go");
        assert_eq!(language_id_from_path("App.java"), "java");
    }

    #[test]
    fn test_language_id_unknown_extension() {
        assert_eq!(language_id_from_path("file.xyz"), "plaintext");
        assert_eq!(language_id_from_path("noext"), "plaintext");
    }

    #[test]
    fn test_parse_lsp_range_valid() {
        let range_json = serde_json::json!({
            "start": { "line": 3, "character": 7 },
            "end": { "line": 3, "character": 15 }
        });
        let range = parse_lsp_range(&range_json).unwrap();
        assert_eq!(range.start_line, 3);
        assert_eq!(range.start_character, 7);
        assert_eq!(range.end_line, 3);
        assert_eq!(range.end_character, 15);
    }

    #[test]
    fn test_parse_lsp_range_missing_fields() {
        let range_json = serde_json::json!({ "start": { "line": 0 } });
        assert!(parse_lsp_range(&range_json).is_none());
    }
}
