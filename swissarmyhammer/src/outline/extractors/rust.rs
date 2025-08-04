//! Rust language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for Rust code,
//! supporting structs, enums, traits, impls, functions, methods, constants,
//! and their associated documentation, visibility, and signature information.

use crate::outline::types::{OutlineNode, OutlineNodeType, SymbolExtractor, Visibility};
use crate::outline::{OutlineError, Result};
use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

/// Rust symbol extractor using Tree-sitter
pub struct RustExtractor {
    /// Tree-sitter queries for different symbol types
    queries: HashMap<OutlineNodeType, Query>,
}

impl RustExtractor {
    /// Create a new Rust extractor with compiled queries
    pub fn new() -> Result<Self> {
        let language = tree_sitter_rust::LANGUAGE.into();
        let mut queries = HashMap::new();

        // Define Tree-sitter queries for each Rust construct
        // Using simpler patterns that are known to work from the search module
        let query_definitions = vec![
            (
                OutlineNodeType::Function,
                r#"(function_item) @function"#,
            ),
            (
                OutlineNodeType::Struct,
                r#"(struct_item) @struct"#,
            ),
            (
                OutlineNodeType::Enum,
                r#"(enum_item) @enum"#,
            ),
            (
                OutlineNodeType::Trait,
                r#"(trait_item) @trait"#,
            ),
            (
                OutlineNodeType::Impl,
                r#"(impl_item) @impl"#,
            ),
            (
                OutlineNodeType::Constant,
                r#"(const_item) @const"#,
            ),
            (
                OutlineNodeType::Variable,
                r#"(static_item) @static"#,
            ),
            (
                OutlineNodeType::TypeAlias,
                r#"(type_item) @type_alias"#,
            ),
            (
                OutlineNodeType::Module,
                r#"(mod_item) @module"#,
            ),
        ];

        // Compile all queries
        for (node_type, query_str) in query_definitions {
            let query = Query::new(&language, query_str).map_err(|e| {
                OutlineError::TreeSitter(format!(
                    "Failed to compile {:?} query: {}",
                    node_type, e
                ))
            })?;
            queries.insert(node_type, query);
        }

        Ok(Self { queries })
    }

    /// Extract the text content of a node
    fn get_node_text(&self, node: &Node, source: &str) -> String {
        source[node.start_byte()..node.end_byte()].to_string()
    }

    /// Extract line numbers for a node (1-based)
    fn get_line_range(&self, node: &Node) -> (usize, usize) {
        let start_line = node.start_position().row + 1;
        let end_line = node.end_position().row + 1;
        (start_line, end_line)
    }

    /// Extract the name from a Tree-sitter node
    fn extract_name_from_node(&self, node: &Node, source: &str) -> Option<String> {
        // Special handling for impl blocks
        if node.kind() == "impl_item" {
            return self.generate_impl_name(node, source);
        }

        // Try to find the name field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(self.get_node_text(&name_node, source));
        }

        // Fallback: look for identifier or type_identifier children
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "identifier" | "type_identifier" => {
                    return Some(self.get_node_text(&child, source));
                }
                _ => continue,
            }
        }

        None
    }

    /// Generate a meaningful name for impl blocks
    fn generate_impl_name(&self, node: &Node, source: &str) -> Option<String> {
        let mut impl_name = String::new();
        let mut found_trait = false;
        let mut found_type = false;

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "type_identifier" => {
                    let type_name = self.get_node_text(&child, source);
                    if !found_trait && !found_type {
                        // First type_identifier might be a trait
                        impl_name = format!("impl {}", type_name);
                        found_trait = true;
                    } else if found_trait && !found_type {
                        // Second type_identifier is the type being implemented for
                        impl_name = format!("{} for {}", impl_name, type_name);
                        found_type = true;
                    }
                }
                "generic_type" => {
                    // Handle generic types in impl blocks
                    let generic_text = self.get_node_text(&child, source);
                    if !found_type {
                        if found_trait {
                            impl_name = format!("{} for {}", impl_name, generic_text);
                        } else {
                            impl_name = format!("impl {}", generic_text);
                        }
                        found_type = true;
                    }
                }
                _ => {}
            }
        }

        if impl_name.is_empty() {
            Some("impl".to_string())
        } else {
            Some(impl_name)
        }
    }

    /// Extract function parameters as a string
    fn extract_function_parameters(&self, node: &Node, source: &str) -> String {
        // Find the parameters node
        for child in node.children(&mut node.walk()) {
            if child.kind() == "parameters" {
                return self.get_node_text(&child, source);
            }
        }
        "()".to_string()
    }

    /// Extract return type annotation
    fn extract_return_type(&self, node: &Node, source: &str) -> Option<String> {
        // Look for return_type field first
        if let Some(return_type_node) = node.child_by_field_name("return_type") {
            return Some(self.get_node_text(&return_type_node, source));
        }
        
        // Fallback: search for any type annotation in children 
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type_annotation" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract generic parameters from a node
    fn extract_generics(&self, node: &Node, source: &str) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type_parameters" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract visibility modifier from a Rust node
    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "visibility_modifier" {
                let vis_text = self.get_node_text(&child, source);
                return match vis_text.as_str() {
                    "pub" => Some(Visibility::Public),
                    "pub(crate)" => Some(Visibility::Package),
                    "pub(super)" => Some(Visibility::Module),
                    s if s.starts_with("pub(in ") => {
                        let path = s.strip_prefix("pub(in ")?.strip_suffix(")")?;
                        Some(Visibility::Custom(path.to_string()))
                    }
                    _ => Some(Visibility::Public), // Default for any pub variant
                };
            }
        }
        None // Private by default in Rust
    }

    /// Extract documentation comments preceding a node
    fn extract_doc_comments(&self, node: &Node, source: &str) -> Option<String> {
        let lines: Vec<&str> = source.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let node_line = node.start_position().row;
        let mut doc_lines = Vec::new();

        // Look backwards from the node's line to find documentation comments
        for line_idx in (0..node_line).rev() {
            let line = lines.get(line_idx)?.trim();
            
            if line.starts_with("///") {
                // Rust doc comment
                let doc_content = line.strip_prefix("///")?.trim();
                doc_lines.insert(0, doc_content);
            } else if line.starts_with("//!") {
                // Module-level doc comment
                let doc_content = line.strip_prefix("//!")?.trim();
                doc_lines.insert(0, doc_content);
            } else if line.is_empty() {
                // Empty line, continue looking
                continue;
            } else {
                // Non-doc comment line, stop looking
                break;
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            Some(doc_lines.join(" "))
        }
    }

    /// Build function signature from components
    fn build_function_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let params = self.extract_function_parameters(node, source);
        let generics = self.extract_generics(node, source);
        let return_type = self.extract_return_type(node, source);

        let mut signature = String::new();
        signature.push_str("fn ");
        signature.push_str(name);
        
        if let Some(gen) = generics {
            signature.push_str(&gen);
        }
        
        signature.push_str(&params);
        
        if let Some(ret) = return_type {
            signature.push_str(" -> ");
            signature.push_str(&ret.strip_prefix(": ").unwrap_or(&ret));
        }

        signature
    }

    /// Build struct signature with generics
    fn build_struct_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("struct ");
        signature.push_str(name);
        
        if let Some(generics) = self.extract_generics(node, source) {
            signature.push_str(&generics);
        }

        signature
    }

    /// Build enum signature with generics
    fn build_enum_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("enum ");
        signature.push_str(name);
        
        if let Some(generics) = self.extract_generics(node, source) {
            signature.push_str(&generics);
        }

        signature
    }

    /// Build trait signature with generics
    fn build_trait_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("trait ");
        signature.push_str(name);
        
        if let Some(generics) = self.extract_generics(node, source) {
            signature.push_str(&generics);
        }

        signature
    }

    /// Build impl signature
    fn build_impl_signature(&self, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("impl");
        
        if let Some(generics) = self.extract_generics(node, source) {
            signature.push(' ');
            signature.push_str(&generics);
        }

        // Look for trait and type names in the impl
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "type_identifier" => {
                    let type_name = self.get_node_text(&child, source);
                    signature.push(' ');
                    signature.push_str(&type_name);
                }
                _ => {}
            }
        }

        signature
    }
}

impl SymbolExtractor for RustExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Result<Vec<OutlineNode>> {
        let mut symbols = Vec::new();
        let root_node = tree.root_node();

        // Process each query type
        for (node_type, query) in &self.queries {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, root_node, source.as_bytes());

            while let Some(query_match) = matches.next() {
                // Get the main captured node (should be the only capture)
                if let Some(capture) = query_match.captures.first() {
                    let node = &capture.node;
                    
                    if let Some(name) = self.extract_name_from_node(node, source) {
                        let (start_line, end_line) = self.get_line_range(node);
                        let mut outline_node = OutlineNode::new(
                            name.clone(),
                            node_type.clone(),
                            start_line,
                            end_line,
                        );

                        // Add signature based on node type
                        let signature = match node_type {
                            OutlineNodeType::Function => {
                                Some(self.build_function_signature(&name, node, source))
                            }
                            OutlineNodeType::Struct => {
                                Some(self.build_struct_signature(&name, node, source))
                            }
                            OutlineNodeType::Enum => {
                                Some(self.build_enum_signature(&name, node, source))
                            }
                            OutlineNodeType::Trait => {
                                Some(self.build_trait_signature(&name, node, source))
                            }
                            OutlineNodeType::Impl => {
                                Some(self.build_impl_signature(node, source))
                            }
                            _ => None,
                        };

                        if let Some(sig) = signature {
                            outline_node = outline_node.with_signature(sig);
                        }

                        // Add visibility
                        if let Some(visibility) = self.parse_visibility(node, source) {
                            outline_node = outline_node.with_visibility(visibility);
                        }

                        // Add documentation
                        if let Some(docs) = self.extract_doc_comments(node, source) {
                            outline_node = outline_node.with_documentation(docs);
                        }

                        symbols.push(outline_node);
                    }
                }
            }
        }

        Ok(symbols)
    }

    fn extract_documentation(&self, node: &Node, source: &str) -> Option<String> {
        self.extract_doc_comments(node, source)
    }

    fn extract_signature(&self, node: &Node, source: &str) -> Option<String> {
        match node.kind() {
            "function_item" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_function_signature(&name, node, source))
                } else {
                    None
                }
            }
            "struct_item" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_struct_signature(&name, node, source))
                } else {
                    None
                }
            }
            "enum_item" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_enum_signature(&name, node, source))
                } else {
                    None
                }
            }
            "trait_item" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_trait_signature(&name, node, source))
                } else {
                    None
                }
            }
            "impl_item" => {
                Some(self.build_impl_signature(node, source))
            }
            _ => None,
        }
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        self.parse_visibility(node, source)
    }

    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        // For now, return symbols as-is
        // TODO: Build proper hierarchical relationships for impl blocks, modules, etc.
        symbols
    }
}

impl Default for RustExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to create RustExtractor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_extractor_creation() {
        let extractor = RustExtractor::new();
        assert!(extractor.is_ok());
    }

    #[test]
    fn test_extract_simple_function() {
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// This is a test function
pub fn hello_world() -> String {
    "Hello, World!".to_string()
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert_eq!(symbols.len(), 1);
        let func = &symbols[0];
        assert_eq!(func.name, "hello_world");
        assert_eq!(func.node_type, OutlineNodeType::Function);
        assert_eq!(func.visibility, Some(Visibility::Public));
        assert!(func.signature.as_ref().unwrap().contains("fn hello_world()"));
        assert!(func.signature.as_ref().unwrap().contains("-> String"));
        assert_eq!(func.documentation.as_ref().unwrap(), "This is a test function");
    }

    #[test] 
    fn test_extract_struct() {
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// A simple struct
pub struct Person {
    name: String,
    age: u32,
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert_eq!(symbols.len(), 1);
        let struct_node = &symbols[0];
        assert_eq!(struct_node.name, "Person");
        assert_eq!(struct_node.node_type, OutlineNodeType::Struct);
        assert_eq!(struct_node.visibility, Some(Visibility::Public));
        assert!(struct_node.signature.as_ref().unwrap().contains("struct Person"));
        assert_eq!(struct_node.documentation.as_ref().unwrap(), "A simple struct");
    }

    #[test]
    fn test_extract_enum() {
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// An enumeration type
pub enum Color {
    Red,
    Green,
    Blue,
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert_eq!(symbols.len(), 1);
        let enum_node = &symbols[0];
        assert_eq!(enum_node.name, "Color");
        assert_eq!(enum_node.node_type, OutlineNodeType::Enum);
        assert_eq!(enum_node.visibility, Some(Visibility::Public));
        assert!(enum_node.signature.as_ref().unwrap().contains("enum Color"));
    }

    #[test]
    fn test_extract_trait() {
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// A trait for displayable objects
pub trait Display {
    fn fmt(&self) -> String;
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert_eq!(symbols.len(), 1);
        let trait_node = &symbols[0];
        assert_eq!(trait_node.name, "Display");
        assert_eq!(trait_node.node_type, OutlineNodeType::Trait);
        assert_eq!(trait_node.visibility, Some(Visibility::Public));
        assert!(trait_node.signature.as_ref().unwrap().contains("trait Display"));
    }

    #[test]
    fn test_extract_multiple_symbols() {
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// A constant value
pub const MAX_SIZE: usize = 100;

/// A structure
pub struct Config {
    name: String,
}

/// A function
pub fn initialize() {
    println!("Initializing");
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert_eq!(symbols.len(), 3);
        
        // Check that we got the expected symbol types
        let types: Vec<&OutlineNodeType> = symbols.iter().map(|s| &s.node_type).collect();
        assert!(types.contains(&&OutlineNodeType::Constant));
        assert!(types.contains(&&OutlineNodeType::Struct));
        assert!(types.contains(&&OutlineNodeType::Function));
    }

    #[test]
    fn test_extract_complex_rust_code() {
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// Module-level documentation
//! This module provides complex Rust constructs

use std::collections::HashMap;
use std::fmt::Display;

/// A constant value
pub const MAX_SIZE: usize = 1000;

/// A static variable
pub static GLOBAL_CONFIG: &str = "default";

/// Generic trait with associated types
pub trait Repository<T>: Send + Sync
where
    T: Clone + Display,
{
    type Error;
    
    /// Save an item to the repository
    fn save(&mut self, item: T) -> Result<(), Self::Error>;
    
    /// Find an item by ID
    fn find(&self, id: &str) -> Option<T>;
}

/// A generic struct with visibility and documentation
pub struct DataProcessor<T> {
    /// Internal data storage
    pub data: HashMap<String, T>,
    /// Configuration settings
    pub(crate) config: ProcessorConfig,
}

/// Configuration enum
#[derive(Debug, Clone)]
pub enum ProcessorConfig {
    /// Fast processing mode
    Fast { threads: usize },
    /// Slow but accurate processing
    Accurate,
    /// Custom configuration
    Custom(String),
}

/// Implementation block with methods
impl<T> DataProcessor<T>
where
    T: Clone + Display,
{
    /// Create a new data processor
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            data: HashMap::new(),
            config,
        }
    }
    
    /// Process an item asynchronously
    pub async fn process_async(&mut self, key: String, value: T) -> Result<(), ProcessError> {
        self.data.insert(key, value);
        Ok(())
    }
}

/// Module definition
pub mod utils {
    /// Helper function
    pub fn format_data(input: &str) -> String {
        format!("formatted: {}", input)
    }
}

/// Type alias for convenience
pub type ProcessResult<T> = Result<T, ProcessError>;

/// Custom error type
#[derive(Debug)]
pub struct ProcessError {
    message: String,
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        // Should extract multiple symbol types
        assert!(!symbols.is_empty());
        
        // Check that we got various types
        let types: std::collections::HashSet<&OutlineNodeType> = symbols.iter().map(|s| &s.node_type).collect();
        
        // Verify we extracted different kinds of symbols
        let expected_types = vec![
            OutlineNodeType::Constant,
            OutlineNodeType::Variable,
            OutlineNodeType::Trait,
            OutlineNodeType::Struct,
            OutlineNodeType::Enum,
            OutlineNodeType::Impl,
            OutlineNodeType::Function,
            OutlineNodeType::Module,
            OutlineNodeType::TypeAlias,
        ];
        
        for expected in expected_types {
            assert!(types.contains(&expected), "Missing symbol type: {:?}", expected);
        }
        
        // Check specific symbols exist with correct names
        let names: Vec<&String> = symbols.iter().map(|s| &s.name).collect();
        assert!(names.contains(&&"MAX_SIZE".to_string()));
        assert!(names.contains(&&"GLOBAL_CONFIG".to_string()));
        assert!(names.contains(&&"Repository".to_string()));
        assert!(names.contains(&&"DataProcessor".to_string()));
        assert!(names.contains(&&"ProcessorConfig".to_string()));
        assert!(names.contains(&&"utils".to_string()));
        assert!(names.contains(&&"ProcessResult".to_string()));
        assert!(names.contains(&&"ProcessError".to_string()));
        
        // Check that public visibility is detected
        let public_symbols: Vec<&OutlineNode> = symbols.iter()
            .filter(|s| s.visibility == Some(Visibility::Public))
            .collect();
        assert!(!public_symbols.is_empty());
        
        // Check that some signatures contain generics
        let has_generics = symbols.iter().any(|s| {
            s.signature.as_ref().map_or(false, |sig| sig.contains("<") && sig.contains(">"))
        });
        assert!(has_generics, "Should find symbols with generic parameters");
        
        // Check that some documentation was extracted
        let has_docs = symbols.iter().any(|s| s.documentation.is_some());
        assert!(has_docs, "Should find symbols with documentation");
        
        println!("Successfully extracted {} symbols from complex Rust code", symbols.len());
        for symbol in &symbols {
            println!("  {:?} '{}' at line {}", symbol.node_type, symbol.name, symbol.start_line);
        }
    }
}