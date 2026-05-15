//! ParsedFile struct containing tree-sitter AST and source text
//!
//! This module provides the core data structure for storing parsed files
//! with both the AST and the original source text for semantic chunking.

use std::path::PathBuf;
use std::sync::Arc;
use tree_sitter::Tree;

/// A parsed file containing both the AST and source text
///
/// This struct holds all the information needed for semantic chunking:
/// - The original source text (for extracting chunks)
/// - The tree-sitter parse tree (for semantic boundaries)
///
/// # Example
///
/// ```ignore
/// use swissarmyhammer_treesitter::ParsedFile;
///
/// // ParsedFile is typically created by the index
/// let parsed = index.get("src/main.rs")?;
/// if let Some(file) = parsed {
///     println!("Lines: {}", file.line_count());
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ParsedFile {
    /// Absolute path to the file
    pub path: PathBuf,

    /// The source text that was parsed (shared ownership for efficiency)
    pub source: Arc<String>,

    /// The tree-sitter parse tree (shared ownership for efficiency)
    pub tree: Arc<Tree>,

    /// MD5 hash of source content for cache invalidation
    pub content_hash: [u8; 16],
}

impl ParsedFile {
    /// Create a new ParsedFile
    pub fn new(path: PathBuf, source: String, tree: Tree, content_hash: [u8; 16]) -> Self {
        Self {
            path,
            source: Arc::new(source),
            tree: Arc::new(tree),
            content_hash,
        }
    }

    /// Check if the parse tree has syntax errors (tree-sitter recovers from errors)
    pub fn has_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// Get a specific line from the source (1-indexed)
    ///
    /// Returns None if the line number is out of bounds.
    pub fn get_line(&self, line_number: usize) -> Option<&str> {
        if line_number == 0 {
            return None;
        }
        self.source.lines().nth(line_number - 1)
    }

    /// Get text for a byte range
    ///
    /// Returns None if the range is out of bounds.
    pub fn get_text(&self, start_byte: usize, end_byte: usize) -> Option<&str> {
        if start_byte > end_byte || end_byte > self.source.len() {
            return None;
        }
        self.source.get(start_byte..end_byte)
    }

    /// Get the root node of the parse tree
    pub fn root_node(&self) -> tree_sitter::Node<'_> {
        self.tree.root_node()
    }

    /// Get the total number of lines in the source
    pub fn line_count(&self) -> usize {
        self.source.lines().count()
    }

    /// Get the total size of the source in bytes
    pub fn byte_count(&self) -> usize {
        self.source.len()
    }

    /// Check if the content hash matches the given hash
    pub fn hash_matches(&self, other_hash: &[u8; 16]) -> bool {
        self.content_hash == *other_hash
    }

    /// Get text for a tree-sitter node
    pub fn node_text(&self, node: tree_sitter::Node<'_>) -> Option<&str> {
        self.get_text(node.start_byte(), node.end_byte())
    }

    /// Get the start line (1-indexed) for a byte offset
    pub fn byte_to_line(&self, byte_offset: usize) -> usize {
        let text_before = &self.source[..byte_offset.min(self.source.len())];
        // Count newlines before this offset and add 1 (line numbers are 1-indexed)
        text_before.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get the start column (1-indexed) for a byte offset
    pub fn byte_to_column(&self, byte_offset: usize) -> usize {
        let text_before = &self.source[..byte_offset.min(self.source.len())];
        match text_before.rfind('\n') {
            Some(pos) => byte_offset - pos,
            None => byte_offset + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageRegistry;

    fn parse_rust_code(source: &str) -> ParsedFile {
        let registry = LanguageRegistry::global();
        let config = registry.get_by_name("rust").unwrap();
        let language = config.language();

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).unwrap();

        let tree = parser.parse(source, None).unwrap();
        let hash = md5::compute(source.as_bytes());

        ParsedFile::new(
            PathBuf::from("test.rs"),
            source.to_string(),
            tree,
            hash.into(),
        )
    }

    #[test]
    fn test_new_parsed_file() {
        let source = "fn main() {}";
        let parsed = parse_rust_code(source);

        assert_eq!(parsed.path, PathBuf::from("test.rs"));
        assert_eq!(parsed.source.as_str(), source);
        assert!(!parsed.has_errors());
    }

    #[test]
    fn test_get_line() {
        let source = "line1\nline2\nline3";
        let parsed = parse_rust_code(source);

        assert_eq!(parsed.get_line(1), Some("line1"));
        assert_eq!(parsed.get_line(2), Some("line2"));
        assert_eq!(parsed.get_line(3), Some("line3"));
        assert_eq!(parsed.get_line(0), None);
        assert_eq!(parsed.get_line(4), None);
    }

    #[test]
    fn test_get_text() {
        let source = "hello world";
        let parsed = parse_rust_code(source);

        assert_eq!(parsed.get_text(0, 5), Some("hello"));
        assert_eq!(parsed.get_text(6, 11), Some("world"));
        assert_eq!(parsed.get_text(0, 11), Some("hello world"));
        assert_eq!(parsed.get_text(5, 3), None); // start > end
        assert_eq!(parsed.get_text(0, 100), None); // end out of bounds
    }

    #[test]
    fn test_root_node() {
        let source = "fn main() {}";
        let parsed = parse_rust_code(source);

        let root = parsed.root_node();
        assert_eq!(root.kind(), "source_file");
    }

    #[test]
    fn test_line_count() {
        let source = "line1\nline2\nline3";
        let parsed = parse_rust_code(source);
        assert_eq!(parsed.line_count(), 3);

        let source = "single line";
        let parsed = parse_rust_code(source);
        assert_eq!(parsed.line_count(), 1);
    }

    #[test]
    fn test_byte_count() {
        let source = "hello";
        let parsed = parse_rust_code(source);
        assert_eq!(parsed.byte_count(), 5);
    }

    #[test]
    fn test_hash_matches() {
        let source = "fn main() {}";
        let parsed = parse_rust_code(source);

        let same_hash = md5::compute(source.as_bytes());
        assert!(parsed.hash_matches(&same_hash.into()));

        let different_hash = md5::compute("different".as_bytes());
        assert!(!parsed.hash_matches(&different_hash.into()));
    }

    #[test]
    fn test_node_text() {
        let source = "fn main() {}";
        let parsed = parse_rust_code(source);

        let root = parsed.root_node();
        assert_eq!(parsed.node_text(root), Some(source));

        // Get the function item
        let func = root.child(0).unwrap();
        assert_eq!(parsed.node_text(func), Some("fn main() {}"));
    }

    #[test]
    fn test_byte_to_line() {
        let source = "line1\nline2\nline3";
        let parsed = parse_rust_code(source);

        assert_eq!(parsed.byte_to_line(0), 1); // Start of line 1
        assert_eq!(parsed.byte_to_line(4), 1); // Still line 1
        assert_eq!(parsed.byte_to_line(6), 2); // Start of line 2
        assert_eq!(parsed.byte_to_line(12), 3); // Start of line 3
    }

    #[test]
    fn test_byte_to_column() {
        let source = "line1\nline2";
        let parsed = parse_rust_code(source);

        assert_eq!(parsed.byte_to_column(0), 1); // First column
        assert_eq!(parsed.byte_to_column(4), 5); // Fifth column (e in line1)
        assert_eq!(parsed.byte_to_column(6), 1); // First column of line 2
    }

    #[test]
    fn test_has_errors_with_valid_code() {
        let source = "fn main() {}";
        let parsed = parse_rust_code(source);
        assert!(!parsed.has_errors());
    }

    #[test]
    fn test_has_errors_with_invalid_code() {
        let source = "fn main( {}"; // Missing closing paren
        let parsed = parse_rust_code(source);
        assert!(parsed.has_errors());
    }

    #[test]
    fn test_arc_sharing() {
        let source = "fn main() {}";
        let parsed = parse_rust_code(source);

        // Clone should share the same underlying data
        let cloned = parsed.clone();

        assert!(Arc::ptr_eq(&parsed.source, &cloned.source));
        assert!(Arc::ptr_eq(&parsed.tree, &cloned.tree));
    }
}
