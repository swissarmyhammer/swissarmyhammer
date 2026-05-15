//! Execute tree-sitter S-expression queries against parsed ASTs.
//!
//! Reads files from disk, parses them with the provided tree-sitter language,
//! runs the S-expression query, and returns captured nodes with file paths,
//! line ranges, and matched text.

use serde::Serialize;
use std::path::Path;
use tree_sitter::StreamingIterator;

use crate::error::CodeContextError;

/// Options for the query AST operation.
#[derive(Debug, Clone)]
pub struct QueryAstOptions {
    /// Maximum number of matches to return.
    pub max_results: usize,
}

impl Default for QueryAstOptions {
    fn default() -> Self {
        Self { max_results: 50 }
    }
}

/// A single capture from a tree-sitter query match.
#[derive(Debug, Clone, Serialize)]
pub struct AstCapture {
    /// Capture name (e.g., "name" from @name)
    pub name: String,
    /// Node kind (e.g., "identifier", "function_item")
    pub kind: String,
    /// Captured text
    pub text: String,
    /// Start line (0-indexed)
    pub start_line: usize,
    /// End line (0-indexed)
    pub end_line: usize,
    /// Start byte offset
    pub start_byte: usize,
    /// End byte offset
    pub end_byte: usize,
}

/// A single match from the query, potentially with multiple captures.
#[derive(Debug, Clone, Serialize)]
pub struct AstMatch {
    /// File path (relative) containing the match
    pub file: String,
    /// Captures from this match
    pub captures: Vec<AstCapture>,
}

/// Result of a query AST operation.
#[derive(Debug, Clone, Serialize)]
pub struct QueryAstResult {
    /// The matches found
    pub matches: Vec<AstMatch>,
    /// Total files scanned
    pub files_scanned: usize,
    /// Whether results were truncated by max_results
    pub truncated: bool,
}

/// Run a tree-sitter S-expression query against files on disk.
///
/// `workspace_root` is used to resolve relative `file_paths` to absolute paths.
/// `language` is the tree-sitter Language to parse with and query against.
/// `file_paths` are relative paths to scan.
/// `query_str` is the S-expression pattern (e.g., `(function_item name: (identifier) @name)`).
pub fn query_ast(
    workspace_root: &Path,
    language: &tree_sitter::Language,
    file_paths: &[String],
    query_str: &str,
    options: &QueryAstOptions,
) -> Result<QueryAstResult, CodeContextError> {
    // Compile the query upfront so we fail fast on bad syntax
    let ts_query = tree_sitter::Query::new(language, query_str)
        .map_err(|e| CodeContextError::QueryError(format!("Invalid S-expression query: {}", e)))?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).map_err(|e| {
        CodeContextError::QueryError(format!("Failed to set parser language: {}", e))
    })?;

    let mut all_matches = Vec::new();
    let mut files_scanned = 0usize;
    let mut truncated = false;

    for relative_path in file_paths {
        let abs_path = workspace_root.join(relative_path);
        let content = match std::fs::read_to_string(&abs_path) {
            Ok(c) => c,
            Err(_) => continue, // skip unreadable files
        };

        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => continue,
        };
        files_scanned += 1;

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&ts_query, tree.root_node(), content.as_bytes());

        while let Some(m) = matches.next() {
            let captures: Vec<AstCapture> = m
                .captures
                .iter()
                .map(|cap| {
                    let node = cap.node;
                    AstCapture {
                        name: ts_query.capture_names()[cap.index as usize].to_string(),
                        kind: node.kind().to_string(),
                        text: content[node.start_byte()..node.end_byte()].to_string(),
                        start_line: node.start_position().row,
                        end_line: node.end_position().row,
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                    }
                })
                .collect();

            if !captures.is_empty() {
                all_matches.push(AstMatch {
                    file: relative_path.clone(),
                    captures,
                });
            }

            if all_matches.len() >= options.max_results {
                truncated = true;
                break;
            }
        }

        if truncated {
            break;
        }
    }

    Ok(QueryAstResult {
        matches: all_matches,
        files_scanned,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_rust_file(dir: &TempDir, name: &str, content: &str) -> String {
        let file_path = dir.path().join(name);
        fs::write(&file_path, content).unwrap();
        name.to_string()
    }

    fn rust_language() -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    #[test]
    fn test_simple_function_query() {
        let dir = TempDir::new().unwrap();
        let file = setup_rust_file(&dir, "test.rs", "fn hello() {}\nfn world() {}\n");

        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[file],
            "(function_item name: (identifier) @name)",
            &QueryAstOptions::default(),
        )
        .unwrap();

        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].captures[0].text, "hello");
        assert_eq!(result.matches[1].captures[0].text, "world");
        assert!(!result.truncated);
    }

    #[test]
    fn test_max_results_truncation() {
        let dir = TempDir::new().unwrap();
        let file = setup_rust_file(
            &dir,
            "test.rs",
            "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\n",
        );

        let opts = QueryAstOptions { max_results: 2 };
        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[file],
            "(function_item name: (identifier) @name)",
            &opts,
        )
        .unwrap();

        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
    }

    #[test]
    fn test_invalid_query_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[],
            "(not_a_valid_node_type @x)",
            &QueryAstOptions::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_nonexistent_file_skipped() {
        let dir = TempDir::new().unwrap();
        let result = query_ast(
            dir.path(),
            &rust_language(),
            &["does_not_exist.rs".to_string()],
            "(function_item name: (identifier) @name)",
            &QueryAstOptions::default(),
        )
        .unwrap();

        assert_eq!(result.files_scanned, 0);
        assert_eq!(result.matches.len(), 0);
    }

    #[test]
    fn test_multiple_captures() {
        let dir = TempDir::new().unwrap();
        let file = setup_rust_file(&dir, "test.rs", "fn greet(name: &str) {}\n");

        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[file],
            "(function_item name: (identifier) @fn_name parameters: (parameters) @params)",
            &QueryAstOptions::default(),
        )
        .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].captures.len(), 2);
        assert_eq!(result.matches[0].captures[0].name, "fn_name");
        assert_eq!(result.matches[0].captures[0].text, "greet");
        assert_eq!(result.matches[0].captures[1].name, "params");
    }

    #[test]
    fn test_multiple_files() {
        let dir = TempDir::new().unwrap();
        let f1 = setup_rust_file(&dir, "a.rs", "fn alpha() {}\n");
        let f2 = setup_rust_file(&dir, "b.rs", "fn beta() {}\n");

        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[f1, f2],
            "(function_item name: (identifier) @name)",
            &QueryAstOptions::default(),
        )
        .unwrap();

        assert_eq!(result.files_scanned, 2);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].captures[0].text, "alpha");
        assert_eq!(result.matches[1].captures[0].text, "beta");
    }

    #[test]
    fn test_capture_line_numbers() {
        let dir = TempDir::new().unwrap();
        let file = setup_rust_file(&dir, "test.rs", "\n\nfn on_line_two() {}\n");

        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[file],
            "(function_item name: (identifier) @name)",
            &QueryAstOptions::default(),
        )
        .unwrap();

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].captures[0].start_line, 2);
    }

    #[test]
    fn test_no_matches() {
        let dir = TempDir::new().unwrap();
        let file = setup_rust_file(&dir, "test.rs", "let x = 42;\n");

        let result = query_ast(
            dir.path(),
            &rust_language(),
            &[file],
            "(function_item name: (identifier) @name)",
            &QueryAstOptions::default(),
        )
        .unwrap();

        assert_eq!(result.files_scanned, 1);
        assert_eq!(result.matches.len(), 0);
    }
}
