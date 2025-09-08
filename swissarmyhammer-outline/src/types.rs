//! Core data structures for outline generation functionality

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Programming language detected for a file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// Rust programming language
    Rust,
    /// Python programming language
    Python,
    /// TypeScript programming language
    TypeScript,
    /// JavaScript programming language
    JavaScript,
    /// Dart programming language
    Dart,
    /// Unknown or unsupported language
    Unknown,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "py" => Self::Python,
            "ts" => Self::TypeScript,
            "js" | "jsx" => Self::JavaScript,
            "tsx" => Self::TypeScript, // TypeScript JSX
            "dart" => Self::Dart,
            _ => Self::Unknown,
        }
    }

    /// Get file extensions associated with this language
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py"],
            Self::TypeScript => &["ts", "tsx"],
            Self::JavaScript => &["js", "jsx"],
            Self::Dart => &["dart"],
            Self::Unknown => &[],
        }
    }

    /// Check if this language is supported for outline generation
    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// A discovered file ready for outline processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredFile {
    /// Full path to the file
    pub path: PathBuf,
    /// Detected programming language
    pub language: Language,
    /// Relative path from the discovery root
    pub relative_path: String,
    /// File size in bytes
    pub size: u64,
}

impl DiscoveredFile {
    /// Create a new DiscoveredFile
    pub fn new(path: PathBuf, language: Language, relative_path: String, size: u64) -> Self {
        Self {
            path,
            language,
            relative_path,
            size,
        }
    }

    /// Check if the file is supported for outline generation
    pub fn is_supported(&self) -> bool {
        self.language.is_supported()
    }

    /// Get the file extension as a string
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|ext| ext.to_str())
    }
}

/// Configuration for file discovery operations
#[derive(Debug, Clone, Default)]
pub struct FileDiscoveryConfig {
    /// Whether to respect .gitignore files
    pub respect_gitignore: bool,
    /// Maximum file size to process (in bytes)
    pub max_file_size: Option<u64>,
    /// Whether to include hidden files
    pub include_hidden: bool,
    /// Custom file extensions to include
    pub custom_extensions: Vec<String>,
}

/// Report of file discovery operation
#[derive(Debug, Clone)]
pub struct FileDiscoveryReport {
    /// Total files discovered
    pub total_files: usize,
    /// Files filtered out (too large, unsupported, etc.)
    pub filtered_files: usize,
    /// Supported files for processing
    pub supported_files: usize,
    /// Time taken for discovery
    pub discovery_time: std::time::Duration,
    /// Patterns used for discovery
    pub patterns: Vec<String>,
}

impl FileDiscoveryReport {
    /// Create a summary string of the discovery results
    pub fn summary(&self) -> String {
        format!(
            "Discovered {} files ({} supported, {} filtered) in {:?} using patterns: {:?}",
            self.total_files,
            self.supported_files,
            self.filtered_files,
            self.discovery_time,
            self.patterns
        )
    }
}

/// Type of outline node/symbol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlineNodeType {
    /// Class definition
    Class,
    /// Interface definition (TypeScript, etc.)
    Interface,
    /// Struct definition (Rust, C, etc.)
    Struct,
    /// Enum definition
    Enum,
    /// Function definition
    Function,
    /// Method within a class or struct
    Method,
    /// Property or field
    Property,
    /// Variable declaration
    Variable,
    /// Module or namespace
    Module,
    /// Type alias
    TypeAlias,
    /// Trait definition (Rust) or protocol
    Trait,
    /// Constant definition
    Constant,
    /// Import or use statement
    Import,
    /// Implementation block (Rust)
    Impl,
}

impl OutlineNodeType {
    /// Get a human-readable name for the node type
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Function => "function",
            Self::Method => "method",
            Self::Property => "property",
            Self::Variable => "variable",
            Self::Module => "module",
            Self::TypeAlias => "type_alias",
            Self::Trait => "trait",
            Self::Constant => "constant",
            Self::Import => "import",
            Self::Impl => "impl",
        }
    }
}

/// Visibility level of a symbol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolVisibility {
    /// Public visibility
    Public,
    /// Private visibility
    Private,
    /// Protected visibility
    Protected,
    /// Package/crate visibility
    Package,
    /// Unknown or not applicable
    Unknown,
}

/// A single symbol/node in the outline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    /// Name of the symbol
    pub name: String,
    /// Type of the symbol
    pub node_type: OutlineNodeType,
    /// Line number where symbol starts
    pub start_line: usize,
    /// Line number where symbol ends
    pub end_line: usize,
    /// Column where symbol starts
    pub start_column: usize,
    /// Column where symbol ends
    pub end_column: usize,
    /// Nested children symbols
    pub children: Vec<Box<OutlineNode>>,
    /// Optional signature (for functions, methods, etc.)
    pub signature: Option<String>,
    /// Optional documentation string
    pub documentation: Option<String>,
    /// Visibility of the symbol
    pub visibility: Option<SymbolVisibility>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl OutlineNode {
    /// Create a new outline node
    pub fn new(name: String, node_type: OutlineNodeType, start_line: usize, end_line: usize) -> Self {
        Self {
            name,
            node_type,
            start_line,
            end_line,
            start_column: 0,
            end_column: 0,
            children: Vec::new(),
            signature: None,
            documentation: None,
            visibility: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, child: OutlineNode) {
        self.children.push(Box::new(child));
    }

    /// Get the total number of symbols (including nested)
    pub fn symbol_count(&self) -> usize {
        1 + self.children.iter().map(|child| child.symbol_count()).sum::<usize>()
    }

    /// Check if this node has children
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// File outline containing all symbols from a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutline {
    /// Path to the source file
    pub file_path: PathBuf,
    /// Programming language of the file
    pub language: Language,
    /// All top-level symbols in the file
    pub symbols: Vec<OutlineNode>,
    /// Time when outline was generated
    pub generated_at: DateTime<Utc>,
    /// Hash of the file content when parsed
    pub content_hash: Option<String>,
}

impl FileOutline {
    /// Create a new file outline
    pub fn new(file_path: PathBuf, language: Language, symbols: Vec<OutlineNode>) -> Self {
        Self {
            file_path,
            language,
            symbols,
            generated_at: Utc::now(),
            content_hash: None,
        }
    }

    /// Get the total number of symbols in this file (including nested)
    pub fn total_symbol_count(&self) -> usize {
        self.symbols.iter().map(|symbol| symbol.symbol_count()).sum()
    }
}

/// Complete outline hierarchy for multiple files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineHierarchy {
    /// All file outlines
    pub files: Vec<FileOutline>,
    /// Metadata about the hierarchy generation
    pub generated_at: DateTime<Utc>,
    /// Total processing time
    pub processing_time: std::time::Duration,
}

impl OutlineHierarchy {
    /// Create a new outline hierarchy
    pub fn new(files: Vec<FileOutline>) -> Self {
        Self {
            files,
            generated_at: Utc::now(),
            processing_time: std::time::Duration::from_secs(0),
        }
    }

    /// Get all files in the hierarchy
    pub fn all_files(&self) -> &[FileOutline] {
        &self.files
    }

    /// Get total symbol count across all files
    pub fn total_symbols(&self) -> usize {
        self.files.iter().map(|file| file.total_symbol_count()).sum()
    }

    /// Get total file count
    pub fn total_files(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("ts"), Language::TypeScript);
        assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
        assert_eq!(Language::from_extension("js"), Language::JavaScript);
        assert_eq!(Language::from_extension("jsx"), Language::JavaScript);
        assert_eq!(Language::from_extension("dart"), Language::Dart);
        assert_eq!(Language::from_extension("unknown"), Language::Unknown);
    }

    #[test]
    fn test_language_support() {
        assert!(Language::Rust.is_supported());
        assert!(Language::Python.is_supported());
        assert!(!Language::Unknown.is_supported());
    }

    #[test]
    fn test_outline_node() {
        let mut node = OutlineNode::new("test_function".to_string(), OutlineNodeType::Function, 10, 20);
        assert_eq!(node.name, "test_function");
        assert_eq!(node.node_type, OutlineNodeType::Function);
        assert_eq!(node.start_line, 10);
        assert_eq!(node.end_line, 20);
        assert!(!node.has_children());
        assert_eq!(node.symbol_count(), 1);

        let child = OutlineNode::new("inner".to_string(), OutlineNodeType::Variable, 15, 15);
        node.add_child(child);
        assert!(node.has_children());
        assert_eq!(node.symbol_count(), 2);
    }

    #[test]
    fn test_discovered_file() {
        let file = DiscoveredFile::new(
            PathBuf::from("/test/file.rs"),
            Language::Rust,
            "file.rs".to_string(),
            1000,
        );
        
        assert_eq!(file.path, PathBuf::from("/test/file.rs"));
        assert_eq!(file.language, Language::Rust);
        assert!(file.is_supported());
        assert_eq!(file.extension(), Some("rs"));
    }
}