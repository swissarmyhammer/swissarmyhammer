//! Core data structures for outline generation functionality

use swissarmyhammer_search::Language;
use chrono::{DateTime, Utc};
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

/// Type of symbol node in the outline tree
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutlineNodeType {
    /// Function definition
    Function,
    /// Method within a class or impl block
    Method,
    /// Class definition
    Class,
    /// Struct definition
    Struct,
    /// Enum definition
    Enum,
    /// Interface definition (TypeScript, Dart)
    Interface,
    /// Trait definition (Rust)
    Trait,
    /// Implementation block (Rust)
    Impl,
    /// Module or namespace
    Module,
    /// Property or field
    Property,
    /// Constant definition
    Constant,
    /// Variable definition
    Variable,
    /// Type alias
    TypeAlias,
    /// Import or use statement
    Import,
}

/// Visibility modifier for symbols
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    /// Public visibility
    Public,
    /// Private visibility
    Private,
    /// Protected visibility
    Protected,
    /// Package/internal visibility
    Package,
    /// Module-level visibility
    Module,
    /// Custom visibility scope
    Custom(String),
}

/// A symbol node in the outline tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    /// Symbol name
    pub name: String,
    /// Type of symbol
    pub node_type: OutlineNodeType,
    /// Starting line number (1-based)
    pub start_line: usize,
    /// Ending line number (1-based)
    pub end_line: usize,
    /// Child symbols (methods in class, etc.)
    pub children: Vec<Box<OutlineNode>>,
    /// Function/method signature
    pub signature: Option<String>,
    /// Documentation comment
    pub documentation: Option<String>,
    /// Visibility modifier
    pub visibility: Option<Visibility>,
    /// Source code range in bytes
    pub source_range: (usize, usize),
}

impl OutlineNode {
    /// Create a new outline node
    pub fn new(
        name: String,
        node_type: OutlineNodeType,
        start_line: usize,
        end_line: usize,
        source_range: (usize, usize),
    ) -> Self {
        Self {
            name,
            node_type,
            start_line,
            end_line,
            children: Vec::new(),
            signature: None,
            documentation: None,
            visibility: None,
            source_range,
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, child: OutlineNode) {
        self.children.push(Box::new(child));
    }

    /// Set the signature
    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Set the documentation
    pub fn with_documentation(mut self, documentation: String) -> Self {
        self.documentation = Some(documentation);
        self
    }

    /// Set the visibility
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Get all symbols in this node and its children (depth-first)
    pub fn all_symbols(&self) -> Vec<&OutlineNode> {
        let mut symbols = vec![self];
        for child in &self.children {
            symbols.extend(child.all_symbols());
        }
        symbols
    }

    /// Check if this node contains a specific line
    pub fn contains_line(&self, line: usize) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// Complete outline tree for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineTree {
    /// File path this outline represents
    pub file_path: PathBuf,
    /// Detected programming language
    pub language: Language,
    /// Root symbols (top-level functions, classes, etc.)
    pub symbols: Vec<OutlineNode>,
    /// Parse timestamp
    pub parsed_at: DateTime<Utc>,
}

impl OutlineTree {
    /// Create a new outline tree
    pub fn new(file_path: PathBuf, language: Language, symbols: Vec<OutlineNode>) -> Self {
        Self {
            file_path,
            language,
            symbols,
            parsed_at: Utc::now(),
        }
    }

    /// Get all symbols in the tree (flattened)
    pub fn all_symbols(&self) -> Vec<&OutlineNode> {
        self.symbols.iter().flat_map(|s| s.all_symbols()).collect()
    }

    /// Find symbols by name
    pub fn find_symbols_by_name(&self, name: &str) -> Vec<&OutlineNode> {
        self.all_symbols()
            .into_iter()
            .filter(|s| s.name == name)
            .collect()
    }

    /// Find symbols by type
    pub fn find_symbols_by_type(&self, node_type: &OutlineNodeType) -> Vec<&OutlineNode> {
        self.all_symbols()
            .into_iter()
            .filter(|s| &s.node_type == node_type)
            .collect()
    }

    /// Find the symbol containing a specific line
    pub fn find_symbol_at_line(&self, line: usize) -> Option<&OutlineNode> {
        self.all_symbols()
            .into_iter()
            .find(|s| s.contains_line(line))
    }

    /// Get summary statistics
    pub fn stats(&self) -> OutlineStats {
        let all_symbols = self.all_symbols();
        let mut stats = OutlineStats::new();

        for symbol in &all_symbols {
            match symbol.node_type {
                OutlineNodeType::Function | OutlineNodeType::Method => stats.functions += 1,
                OutlineNodeType::Class | OutlineNodeType::Struct => stats.classes += 1,
                OutlineNodeType::Enum => stats.enums += 1,
                OutlineNodeType::Interface | OutlineNodeType::Trait => stats.interfaces += 1,
                OutlineNodeType::Impl => stats.classes += 1, // Count impls as classes
                OutlineNodeType::Module => stats.modules += 1,
                OutlineNodeType::Constant => stats.constants += 1,
                OutlineNodeType::Variable => stats.variables += 1,
                OutlineNodeType::Property => stats.properties += 1,
                OutlineNodeType::TypeAlias => stats.type_aliases += 1,
                OutlineNodeType::Import => stats.imports += 1,
            }
        }

        stats.total = all_symbols.len();
        stats
    }
}

/// Statistics about an outline tree
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutlineStats {
    /// Total number of symbols
    pub total: usize,
    /// Number of functions/methods
    pub functions: usize,
    /// Number of classes/structs
    pub classes: usize,
    /// Number of enums
    pub enums: usize,
    /// Number of interfaces
    pub interfaces: usize,
    /// Number of modules
    pub modules: usize,
    /// Number of constants
    pub constants: usize,
    /// Number of variables
    pub variables: usize,
    /// Number of properties
    pub properties: usize,
    /// Number of type aliases
    pub type_aliases: usize,
    /// Number of imports
    pub imports: usize,
}

impl OutlineStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Format as summary string
    pub fn summary(&self) -> String {
        format!(
            "{} total symbols: {} functions, {} classes, {} enums, {} imports",
            self.total, self.functions, self.classes, self.enums, self.imports
        )
    }
}
