//! Core data structures for outline generation functionality

use crate::search::types::Language;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
        !matches!(self.language, Language::Unknown)
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
    /// Maximum depth for directory traversal
    pub max_depth: Option<usize>,
}

impl FileDiscoveryConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self {
            respect_gitignore: true,
            max_file_size: Some(10 * 1024 * 1024), // 10MB default
            include_hidden: false,
            max_depth: None,
        }
    }

    /// Enable gitignore processing
    pub fn with_gitignore(mut self, enabled: bool) -> Self {
        self.respect_gitignore = enabled;
        self
    }

    /// Set maximum file size
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = Some(size);
        self
    }

    /// Enable hidden file inclusion
    pub fn with_hidden_files(mut self, enabled: bool) -> Self {
        self.include_hidden = enabled;
        self
    }

    /// Set maximum traversal depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

/// Report from a file discovery operation
#[derive(Debug, Clone, Default)]
pub struct FileDiscoveryReport {
    /// Total number of files discovered
    pub files_discovered: usize,
    /// Number of supported files
    pub supported_files: usize,
    /// Number of unsupported files
    pub unsupported_files: usize,
    /// Number of files skipped due to size limits
    pub files_skipped_size: usize,
    /// Number of files skipped due to gitignore
    pub files_skipped_ignored: usize,
    /// Total bytes of discovered files
    pub total_bytes: u64,
    /// Time taken for discovery
    pub duration: std::time::Duration,
    /// Errors encountered during discovery
    pub errors: Vec<(PathBuf, String)>,
}

impl FileDiscoveryReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a discovered file to the report
    pub fn add_file(&mut self, file: &DiscoveredFile) {
        self.files_discovered += 1;
        self.total_bytes += file.size;

        if file.is_supported() {
            self.supported_files += 1;
        } else {
            self.unsupported_files += 1;
        }
    }

    /// Add a skipped file due to size
    pub fn add_skipped_size(&mut self, _path: &Path, size: u64) {
        self.files_skipped_size += 1;
        self.total_bytes += size;
    }

    /// Add a skipped file due to gitignore
    pub fn add_skipped_ignored(&mut self, _path: &Path) {
        self.files_skipped_ignored += 1;
    }

    /// Add an error to the report
    pub fn add_error(&mut self, path: PathBuf, error: String) {
        self.errors.push((path, error));
    }

    /// Get a summary string of the discovery results
    pub fn summary(&self) -> String {
        format!(
            "Discovered {} files ({} supported, {} unsupported), {} skipped, {} errors, {:.1} MB total",
            self.files_discovered,
            self.supported_files,
            self.unsupported_files,
            self.files_skipped_size + self.files_skipped_ignored,
            self.errors.len(),
            self.total_bytes as f64 / (1024.0 * 1024.0)
        )
    }
}

/// Type of outline node representing different code constructs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutlineNodeType {
    /// Function definition
    Function,
    /// Method definition within a class/impl/trait
    Method,
    /// Class definition
    Class,
    /// Struct definition (Rust)
    Struct,
    /// Enum definition
    Enum,
    /// Interface definition (TypeScript)
    Interface,
    /// Trait definition (Rust)
    Trait,
    /// Implementation block (Rust)
    Impl,
    /// Module/namespace definition
    Module,
    /// Property/field definition
    Property,
    /// Constant definition
    Constant,
    /// Variable definition
    Variable,
    /// Type alias definition
    TypeAlias,
}

/// Visibility modifier for code elements
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    /// Public visibility
    Public,
    /// Private visibility (default in most languages)
    Private,
    /// Protected visibility
    Protected,
    /// Package/crate visibility (Rust pub(crate))
    Package,
    /// Module visibility (Rust pub(super))
    Module,
    /// Custom visibility path (Rust pub(in path))
    Custom(String),
}

/// A single node in the code outline tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    /// Name of the code element
    pub name: String,
    /// Type of the code element
    pub node_type: OutlineNodeType,
    /// Starting line number (1-based)
    pub start_line: usize,
    /// Ending line number (1-based)
    pub end_line: usize,
    /// Function/method signature with types (optional)
    pub signature: Option<String>,
    /// Type information for properties/variables (optional)
    pub type_info: Option<String>,
    /// Documentation comment text (optional)
    pub documentation: Option<String>,
    /// Visibility modifier (optional)
    pub visibility: Option<Visibility>,
    /// Child nodes for nested structures
    pub children: Vec<OutlineNode>,
}

impl OutlineNode {
    /// Create a new outline node with basic information
    pub fn new(
        name: String,
        node_type: OutlineNodeType,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        Self {
            name,
            node_type,
            start_line,
            end_line,
            signature: None,
            type_info: None,
            documentation: None,
            visibility: None,
            children: Vec::new(),
        }
    }

    /// Set the signature for this node
    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Set the type information for this node
    pub fn with_type_info(mut self, type_info: String) -> Self {
        self.type_info = Some(type_info);
        self
    }

    /// Set the documentation for this node
    pub fn with_documentation(mut self, documentation: String) -> Self {
        self.documentation = Some(documentation);
        self
    }

    /// Set the visibility for this node
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Add a child node
    pub fn add_child(mut self, child: OutlineNode) -> Self {
        self.children.push(child);
        self
    }

    /// Add multiple child nodes
    pub fn with_children(mut self, children: Vec<OutlineNode>) -> Self {
        self.children.extend(children);
        self
    }
}

/// Complete outline for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutline {
    /// Path to the source file
    pub file_path: PathBuf,
    /// Programming language
    pub language: Language,
    /// Root-level symbols in the file
    pub symbols: Vec<OutlineNode>,
}

impl FileOutline {
    /// Create a new file outline
    pub fn new(file_path: PathBuf, language: Language) -> Self {
        Self {
            file_path,
            language,
            symbols: Vec::new(),
        }
    }

    /// Add a symbol to this file outline
    pub fn add_symbol(mut self, symbol: OutlineNode) -> Self {
        self.symbols.push(symbol);
        self
    }

    /// Add multiple symbols to this file outline
    pub fn with_symbols(mut self, symbols: Vec<OutlineNode>) -> Self {
        self.symbols.extend(symbols);
        self
    }
}

/// Trait for extracting symbols from source code using Tree-sitter
pub trait SymbolExtractor: Send + Sync {
    /// Extract symbols from a Tree-sitter syntax tree
    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &str) -> crate::outline::Result<Vec<OutlineNode>>;
    
    /// Extract documentation comment for a specific node
    fn extract_documentation(&self, node: &tree_sitter::Node, source: &str) -> Option<String>;
    
    /// Extract signature information for a function/method node
    fn extract_signature(&self, node: &tree_sitter::Node, source: &str) -> Option<String>;
    
    /// Extract visibility modifier from a node
    fn extract_visibility(&self, node: &tree_sitter::Node, source: &str) -> Option<Visibility>;
    
    /// Build hierarchical relationships between symbols
    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        // Default implementation returns symbols as-is
        // Language-specific extractors can override this
        symbols
    }
}
