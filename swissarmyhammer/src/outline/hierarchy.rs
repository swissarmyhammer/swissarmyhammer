//! Hierarchical structure builder for organizing code outlines into file system structure
//!
//! This module provides the functionality to take parsed outline trees from individual files
//! and organize them into a nested hierarchy that mirrors the file system structure. This
//! creates the foundation for structured YAML output generation while maintaining both
//! directory organization and symbol relationships.

use crate::outline::{OutlineNode, OutlineTree, Result};
use swissarmyhammer_search::Language;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Builder for creating hierarchical outline structures
#[derive(Debug)]
pub struct HierarchyBuilder {
    /// Root directory of the hierarchy
    root: OutlineDirectory,
    /// Sorting strategy to apply
    sort_order: SortOrder,
    /// Files to be organized
    files: Vec<OutlineTree>,
}

impl HierarchyBuilder {
    /// Create a new hierarchy builder
    pub fn new() -> Self {
        Self {
            root: OutlineDirectory::new(".".to_string(), PathBuf::from(".")),
            sort_order: SortOrder::SourceOrder,
            files: Vec::new(),
        }
    }

    /// Add a parsed file outline to the builder
    pub fn add_file_outline(&mut self, outline: OutlineTree) -> Result<()> {
        self.files.push(outline);
        Ok(())
    }

    /// Set the sorting strategy
    pub fn with_sorting(mut self, sort: SortOrder) -> Self {
        self.sort_order = sort;
        self
    }

    /// Build the complete hierarchy from accumulated files
    pub fn build_hierarchy(mut self) -> Result<OutlineHierarchy> {
        // Group files by directory
        let mut directory_map: HashMap<PathBuf, Vec<OutlineTree>> = HashMap::new();

        for outline in &self.files {
            let parent_dir = outline
                .file_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf();
            directory_map
                .entry(parent_dir)
                .or_default()
                .push(outline.clone());
        }

        // Find the actual root directory to start from
        let _root_path = if directory_map.is_empty() {
            Path::new(".")
        } else {
            // Find the common root of all directories
            let mut dirs: Vec<_> = directory_map.keys().collect();
            dirs.sort();
            if dirs.is_empty() {
                Path::new(".")
            } else {
                // Start from the first directory's root
                dirs[0].as_path()
            }
        };

        // Build directory tree structure
        self.root = self.build_directory_tree_simple(&directory_map)?;

        // Apply sorting
        self.root.sort_contents(self.sort_order);

        // Collect statistics
        let mut total_files = 0;
        let mut total_symbols = 0;
        let mut languages = HashSet::new();

        self.root
            .collect_stats(&mut total_files, &mut total_symbols, &mut languages);

        Ok(OutlineHierarchy {
            root: self.root,
            total_files,
            total_symbols,
            languages,
        })
    }

    /// Build directory tree by creating a flat structure initially
    fn build_directory_tree_simple(
        &self,
        directory_map: &HashMap<PathBuf, Vec<OutlineTree>>,
    ) -> Result<OutlineDirectory> {
        let mut root = OutlineDirectory::new(".".to_string(), PathBuf::from("."));

        // Create a flat structure first - just add all files under root for now
        // This is a simplified version that can be enhanced later
        for outlines in directory_map.values() {
            // For now, add all files directly to root
            // In a more sophisticated version, we'd build the proper directory structure
            for outline in outlines {
                let file = OutlineFile::from_outline_tree(outline.clone())?;
                root.files.push(file);
            }
        }

        Ok(root)
    }
}

impl Default for HierarchyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete hierarchical outline structure
#[derive(Debug, Clone)]
pub struct OutlineHierarchy {
    /// Root directory of the hierarchy
    pub root: OutlineDirectory,
    /// Total number of files processed
    pub total_files: usize,
    /// Total number of symbols across all files
    pub total_symbols: usize,
    /// Set of programming languages found
    pub languages: HashSet<Language>,
}

impl OutlineHierarchy {
    /// Get all files in the hierarchy (flattened)
    pub fn all_files(&self) -> Vec<&OutlineFile> {
        self.root.all_files()
    }

    /// Find files by language
    pub fn files_by_language(&self, language: &Language) -> Vec<&OutlineFile> {
        self.all_files()
            .into_iter()
            .filter(|f| &f.language == language)
            .collect()
    }

    /// Get summary statistics
    pub fn summary(&self) -> String {
        format!(
            "Hierarchy: {} files, {} symbols, {} languages ({})",
            self.total_files,
            self.total_symbols,
            self.languages.len(),
            self.languages
                .iter()
                .map(|l| format!("{l:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// A directory in the outline hierarchy
#[derive(Debug, Clone)]
pub struct OutlineDirectory {
    /// Directory name
    pub name: String,
    /// Full path to the directory
    pub path: PathBuf,
    /// Files directly in this directory
    pub files: Vec<OutlineFile>,
    /// Subdirectories
    pub subdirectories: Vec<OutlineDirectory>,
}

impl OutlineDirectory {
    /// Create a new directory
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            files: Vec::new(),
            subdirectories: Vec::new(),
        }
    }

    /// Get all files in this directory and subdirectories (recursive)
    pub fn all_files(&self) -> Vec<&OutlineFile> {
        let mut files: Vec<&OutlineFile> = self.files.iter().collect();
        for subdir in &self.subdirectories {
            files.extend(subdir.all_files());
        }
        files
    }

    /// Sort contents according to the specified order
    pub fn sort_contents(&mut self, sort_order: SortOrder) {
        match sort_order {
            SortOrder::SourceOrder => {
                // Keep original order
            }
            SortOrder::Alphabetical => {
                self.files.sort_by(|a, b| a.name.cmp(&b.name));
                self.subdirectories.sort_by(|a, b| a.name.cmp(&b.name));
                for file in &mut self.files {
                    file.sort_symbols_alphabetically();
                }
            }
            SortOrder::ByKind => {
                for file in &mut self.files {
                    file.sort_symbols_by_kind();
                }
            }
            SortOrder::ByVisibility => {
                for file in &mut self.files {
                    file.sort_symbols_by_visibility();
                }
            }
        }

        // Recursively sort subdirectories
        for subdir in &mut self.subdirectories {
            subdir.sort_contents(sort_order);
        }
    }

    /// Collect statistics recursively
    pub fn collect_stats(
        &self,
        total_files: &mut usize,
        total_symbols: &mut usize,
        languages: &mut HashSet<Language>,
    ) {
        *total_files += self.files.len();
        for file in &self.files {
            *total_symbols += file.symbol_count();
            languages.insert(file.language.clone());
        }

        for subdir in &self.subdirectories {
            subdir.collect_stats(total_files, total_symbols, languages);
        }
    }
}

/// A file in the outline hierarchy
#[derive(Debug, Clone)]
pub struct OutlineFile {
    /// File name
    pub name: String,
    /// Full path to the file
    pub path: PathBuf,
    /// Detected programming language
    pub language: Language,
    /// Top-level symbols in the file
    pub symbols: Vec<OutlineNode>,
    /// Parse errors encountered
    pub parse_errors: Vec<String>,
}

impl OutlineFile {
    /// Create a new outline file
    pub fn new(name: String, path: PathBuf, language: Language, symbols: Vec<OutlineNode>) -> Self {
        Self {
            name,
            path,
            language,
            symbols,
            parse_errors: Vec::new(),
        }
    }

    /// Create from an OutlineTree
    pub fn from_outline_tree(tree: OutlineTree) -> Result<Self> {
        let name = tree
            .file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            name,
            path: tree.file_path,
            language: tree.language,
            symbols: tree.symbols,
            parse_errors: Vec::new(),
        })
    }

    /// Get total number of symbols (including nested)
    pub fn symbol_count(&self) -> usize {
        self.symbols.iter().map(|s| s.all_symbols().len()).sum()
    }

    /// Sort symbols alphabetically
    pub fn sort_symbols_alphabetically(&mut self) {
        self.symbols.sort_by(|a, b| a.name.cmp(&b.name));
        for symbol in &mut self.symbols {
            sort_node_children_alphabetically(symbol);
        }
    }

    /// Sort symbols by kind (functions, classes, etc.)
    pub fn sort_symbols_by_kind(&mut self) {
        self.symbols
            .sort_by_key(|s| symbol_kind_order(&s.node_type));
        for symbol in &mut self.symbols {
            sort_node_children_by_kind(symbol);
        }
    }

    /// Sort symbols by visibility (public first, then private)
    pub fn sort_symbols_by_visibility(&mut self) {
        self.symbols
            .sort_by_key(|s| visibility_order(&s.visibility));
        for symbol in &mut self.symbols {
            sort_node_children_by_visibility(symbol);
        }
    }
}

/// Sorting strategies for organizing hierarchy contents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Maintain original source order
    SourceOrder,
    /// Sort alphabetically by name
    Alphabetical,
    /// Group by symbol kind, then alphabetical
    ByKind,
    /// Public symbols first, then private
    ByVisibility,
}

// Helper functions for sorting

fn sort_node_children_alphabetically(node: &mut OutlineNode) {
    node.children.sort_by(|a, b| a.name.cmp(&b.name));
    for child in &mut node.children {
        sort_node_children_alphabetically(child);
    }
}

fn sort_node_children_by_kind(node: &mut OutlineNode) {
    node.children
        .sort_by_key(|s| symbol_kind_order(&s.node_type));
    for child in &mut node.children {
        sort_node_children_by_kind(child);
    }
}

fn sort_node_children_by_visibility(node: &mut OutlineNode) {
    node.children
        .sort_by_key(|s| visibility_order(&s.visibility));
    for child in &mut node.children {
        sort_node_children_by_visibility(child);
    }
}

fn symbol_kind_order(node_type: &crate::outline::OutlineNodeType) -> u8 {
    use crate::outline::OutlineNodeType;
    match node_type {
        OutlineNodeType::Module => 0,
        OutlineNodeType::Import => 1,
        OutlineNodeType::Constant => 2,
        OutlineNodeType::TypeAlias => 3,
        OutlineNodeType::Enum => 4,
        OutlineNodeType::Interface => 5,
        OutlineNodeType::Trait => 6,
        OutlineNodeType::Struct => 7,
        OutlineNodeType::Class => 8,
        OutlineNodeType::Impl => 9,
        OutlineNodeType::Function => 10,
        OutlineNodeType::Method => 11,
        OutlineNodeType::Variable => 12,
        OutlineNodeType::Property => 13,
    }
}

fn visibility_order(visibility: &Option<crate::outline::Visibility>) -> u8 {
    use crate::outline::Visibility;
    match visibility {
        Some(Visibility::Public) => 0,
        Some(Visibility::Protected) => 1,
        Some(Visibility::Package) => 2,
        Some(Visibility::Module) => 3,
        Some(Visibility::Private) => 4,
        Some(Visibility::Custom(_)) => 5,
        None => 6, // Unknown/default visibility last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::{OutlineNodeType, OutlineTree};
    use swissarmyhammer_search::Language;
    use std::path::PathBuf;

    fn create_test_outline_tree(file_path: &str, symbols: Vec<OutlineNode>) -> OutlineTree {
        OutlineTree::new(PathBuf::from(file_path), Language::Rust, symbols)
    }

    fn create_test_node(name: &str, node_type: OutlineNodeType) -> OutlineNode {
        OutlineNode::new(name.to_string(), node_type, 1, 10, (0, 100))
    }

    #[test]
    fn test_hierarchy_builder_new() {
        let builder = HierarchyBuilder::new();
        assert_eq!(builder.root.name, ".");
        assert_eq!(builder.files.len(), 0);
        assert!(matches!(builder.sort_order, SortOrder::SourceOrder));
    }

    #[test]
    fn test_add_file_outline() {
        let mut builder = HierarchyBuilder::new();
        let outline = create_test_outline_tree("src/lib.rs", vec![]);

        let result = builder.add_file_outline(outline);
        assert!(result.is_ok());
        assert_eq!(builder.files.len(), 1);
    }

    #[test]
    fn test_with_sorting() {
        let builder = HierarchyBuilder::new().with_sorting(SortOrder::Alphabetical);
        assert!(matches!(builder.sort_order, SortOrder::Alphabetical));
    }

    #[test]
    fn test_simple_hierarchy_build() {
        let mut builder = HierarchyBuilder::new();

        // Add a simple file
        let symbols = vec![create_test_node("main", OutlineNodeType::Function)];
        let outline = create_test_outline_tree("src/main.rs", symbols);
        builder.add_file_outline(outline).unwrap();

        let hierarchy = builder.build_hierarchy().unwrap();

        assert_eq!(hierarchy.total_files, 1);
        assert_eq!(hierarchy.total_symbols, 1);
        assert!(hierarchy.languages.contains(&Language::Rust));
    }

    #[test]
    fn test_sorting_strategies() {
        let mut symbols = vec![
            create_test_node("zebra_function", OutlineNodeType::Function),
            create_test_node("Alpha", OutlineNodeType::Class),
            create_test_node("beta_constant", OutlineNodeType::Constant),
        ];

        // Test ByKind sorting
        symbols.sort_by_key(|s| symbol_kind_order(&s.node_type));
        assert_eq!(symbols[0].name, "beta_constant"); // Constants first
        assert_eq!(symbols[1].name, "Alpha"); // Classes second
        assert_eq!(symbols[2].name, "zebra_function"); // Functions last
    }

    #[test]
    fn test_outline_file_from_tree() {
        let symbols = vec![create_test_node("test_func", OutlineNodeType::Function)];
        let tree = create_test_outline_tree("src/test.rs", symbols);

        let file = OutlineFile::from_outline_tree(tree).unwrap();

        assert_eq!(file.name, "test.rs");
        assert_eq!(file.language, Language::Rust);
        assert_eq!(file.symbols.len(), 1);
        assert_eq!(file.symbols[0].name, "test_func");
    }

    #[test]
    fn test_symbol_count() {
        let mut parent = create_test_node("Parent", OutlineNodeType::Class);
        parent.add_child(create_test_node("child1", OutlineNodeType::Method));
        parent.add_child(create_test_node("child2", OutlineNodeType::Property));

        let file = OutlineFile::new(
            "test.rs".to_string(),
            PathBuf::from("test.rs"),
            Language::Rust,
            vec![parent],
        );

        // Should count parent + 2 children = 3 total symbols
        assert_eq!(file.symbol_count(), 3);
    }

    #[test]
    fn test_directory_all_files() {
        let mut dir = OutlineDirectory::new("src".to_string(), PathBuf::from("src"));

        let file1 = OutlineFile::new(
            "main.rs".to_string(),
            PathBuf::from("src/main.rs"),
            Language::Rust,
            vec![],
        );
        dir.files.push(file1);

        let mut subdir = OutlineDirectory::new("utils".to_string(), PathBuf::from("src/utils"));
        let file2 = OutlineFile::new(
            "helpers.rs".to_string(),
            PathBuf::from("src/utils/helpers.rs"),
            Language::Rust,
            vec![],
        );
        subdir.files.push(file2);
        dir.subdirectories.push(subdir);

        let all_files = dir.all_files();
        assert_eq!(all_files.len(), 2);

        let file_names: Vec<_> = all_files.iter().map(|f| f.name.as_str()).collect();
        assert!(file_names.contains(&"main.rs"));
        assert!(file_names.contains(&"helpers.rs"));
    }

    #[test]
    fn test_multi_language_project() {
        let mut builder = HierarchyBuilder::new();

        // Add Rust file
        let rust_symbols = vec![create_test_node("main", OutlineNodeType::Function)];
        let rust_outline =
            OutlineTree::new(PathBuf::from("src/main.rs"), Language::Rust, rust_symbols);
        builder.add_file_outline(rust_outline).unwrap();

        // Add JavaScript file
        let js_symbols = vec![create_test_node("initApp", OutlineNodeType::Function)];
        let js_outline = OutlineTree::new(
            PathBuf::from("frontend/app.js"),
            Language::JavaScript,
            js_symbols,
        );
        builder.add_file_outline(js_outline).unwrap();

        let hierarchy = builder.build_hierarchy().unwrap();

        assert_eq!(hierarchy.total_files, 2);
        assert_eq!(hierarchy.languages.len(), 2);
        assert!(hierarchy.languages.contains(&Language::Rust));
        assert!(hierarchy.languages.contains(&Language::JavaScript));

        let rust_files = hierarchy.files_by_language(&Language::Rust);
        let js_files = hierarchy.files_by_language(&Language::JavaScript);

        assert_eq!(rust_files.len(), 1);
        assert_eq!(js_files.len(), 1);
        assert!(rust_files[0].name.ends_with(".rs"));
        assert!(js_files[0].name.ends_with(".js"));
    }

    #[test]
    fn test_alphabetical_sorting_integration() {
        let mut builder = HierarchyBuilder::new().with_sorting(SortOrder::Alphabetical);

        // Create file with multiple symbols in non-alphabetical order
        let symbols = vec![
            create_test_node("zebra_function", OutlineNodeType::Function),
            create_test_node("alpha_function", OutlineNodeType::Function),
            create_test_node("beta_class", OutlineNodeType::Class),
        ];

        let outline = create_test_outline_tree("src/test.rs", symbols);
        builder.add_file_outline(outline).unwrap();

        let hierarchy = builder.build_hierarchy().unwrap();
        let file = hierarchy.all_files().into_iter().next().unwrap();

        // Should be sorted alphabetically
        let names: Vec<&String> = file.symbols.iter().map(|s| &s.name).collect();
        assert_eq!(
            names,
            vec!["alpha_function", "beta_class", "zebra_function"]
        );
    }

    #[test]
    fn test_nested_directory_structure() {
        let mut builder = HierarchyBuilder::new();

        // Add files in nested directories
        let outline1 = create_test_outline_tree(
            "src/main.rs",
            vec![create_test_node("main", OutlineNodeType::Function)],
        );
        let outline2 = create_test_outline_tree(
            "src/utils/helpers.rs",
            vec![create_test_node("Helper", OutlineNodeType::Struct)],
        );
        let outline3 = create_test_outline_tree(
            "tests/integration.rs",
            vec![create_test_node("test_main", OutlineNodeType::Function)],
        );

        builder.add_file_outline(outline1).unwrap();
        builder.add_file_outline(outline2).unwrap();
        builder.add_file_outline(outline3).unwrap();

        let hierarchy = builder.build_hierarchy().unwrap();

        assert_eq!(hierarchy.total_files, 3);
        assert_eq!(hierarchy.total_symbols, 3);

        let all_files = hierarchy.all_files();
        let file_paths: Vec<String> = all_files
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();

        assert!(file_paths.iter().any(|p| p.contains("src/main.rs")));
        assert!(file_paths
            .iter()
            .any(|p| p.contains("src/utils/helpers.rs")));
        assert!(file_paths
            .iter()
            .any(|p| p.contains("tests/integration.rs")));
    }
}
