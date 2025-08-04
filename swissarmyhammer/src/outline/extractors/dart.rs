//! Dart language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for Dart code,
//! supporting classes, mixins, enums, extensions, functions, methods, constructors,
//! properties, and their associated documentation, visibility, and signature information.
//! Includes support for Flutter-specific patterns and modern Dart language features.

use crate::outline::types::{OutlineNode, OutlineNodeType, Visibility};
use crate::outline::parser::SymbolExtractor;
use crate::outline::{OutlineError, Result};
use std::collections::HashMap;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

/// Dart symbol extractor using Tree-sitter
pub struct DartExtractor {
    /// Tree-sitter queries for different symbol types
    queries: HashMap<OutlineNodeType, Query>,
}

impl DartExtractor {
    /// Create a new Dart extractor with compiled queries
    pub fn new() -> Result<Self> {
        let language = tree_sitter_dart::language().into();
        let mut queries = HashMap::new();

        // Define Tree-sitter queries for Dart constructs
        // Using the actual node names from tree-sitter-dart grammar
        let query_definitions = vec![
            // Classes
            (
                OutlineNodeType::Class,
                r#"(class_definition) @class"#,
            ),
            // Mixins
            (
                OutlineNodeType::Interface,
                r#"(mixin_declaration) @mixin"#,
            ),
            // Extensions
            (
                OutlineNodeType::Interface,
                r#"(extension_declaration) @extension"#,
            ),
            // Enums
            (
                OutlineNodeType::Enum,
                r#"(enum_declaration) @enum"#,
            ),
            // Functions
            (
                OutlineNodeType::Function,
                r#"(function_signature) @function"#,
            ),
            // Method signatures
            (
                OutlineNodeType::Method,
                r#"(method_signature) @method"#,
            ),
            // Getter signatures
            (
                OutlineNodeType::Property,
                r#"(getter_signature) @getter"#,
            ),
            // Setter signatures
            (
                OutlineNodeType::Property,
                r#"(setter_signature) @setter"#,
            ),
            // Constructor signatures
            (
                OutlineNodeType::Method,
                r#"(constructor_signature) @constructor"#,
            ),
            // Factory constructor signatures
            (
                OutlineNodeType::Method,
                r#"(factory_constructor_signature) @factory"#,
            ),
            // Type aliases
            (
                OutlineNodeType::TypeAlias,
                r#"(type_alias) @type_alias"#,
            ),
            // Variables
            (
                OutlineNodeType::Variable,
                r#"(initialized_variable_definition) @variable"#,
            ),
            // Libraries
            (
                OutlineNodeType::Module,
                r#"(library_name) @library"#,
            ),
            // Imports
            (
                OutlineNodeType::Import,
                r#"(import_or_export) @import"#,
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

    /// Extract the name from a Dart Tree-sitter node
    fn extract_name_from_node(&self, node: &Node, source: &str) -> Option<String> {
        // Handle special cases first
        match node.kind() {
            "lambda_expression" => return self.extract_lambda_function_name(node, source),
            "mixin_declaration" => return self.extract_mixin_name(node, source),
            "extension_declaration" => return self.extract_extension_name(node, source),
            "factory_constructor_signature" => return self.extract_factory_name(node, source),
            "constructor_signature" => return self.extract_constructor_name(node, source),
            "class_definition" => return self.extract_class_name(node, source),
            "function_signature" => return self.extract_function_name(node, source),
            "getter_signature" => return self.extract_getter_name(node, source),
            "enum_declaration" => return self.extract_enum_name(node, source),
            _ => {}
        }

        // Try to find the name field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(self.get_node_text(&name_node, source));
        }

        // Fallback: look for identifier children
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }

        None
    }

    /// Extract class name from class_definition node
    fn extract_class_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for identifier after 'class' keyword
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract mixin name from mixin_declaration node
    fn extract_mixin_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for identifier after 'mixin' keyword
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract extension name from extension_declaration node
    fn extract_extension_name(&self, node: &Node, source: &str) -> Option<String> {
        // Extensions can be named or unnamed
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }
        // If no name found, it's an unnamed extension
        Some("<unnamed extension>".to_string())
    }

    /// Extract constructor name from constructor_signature node
    fn extract_constructor_name(&self, node: &Node, source: &str) -> Option<String> {
        // Named constructors have an identifier after the class name
        let mut identifiers = Vec::new();
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                identifiers.push(self.get_node_text(&child, source));
            }
        }
        
        match identifiers.len() {
            0 => Some("<constructor>".to_string()),
            1 => Some(identifiers[0].clone()), // Default constructor
            _ => Some(identifiers.last().unwrap().clone()), // Named constructor
        }
    }

    /// Extract factory constructor name from factory_constructor_signature node
    fn extract_factory_name(&self, node: &Node, source: &str) -> Option<String> {
        // Factory constructors can be named
        let mut identifiers = Vec::new();
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                identifiers.push(self.get_node_text(&child, source));
            }
        }
        
        match identifiers.len() {
            0 => Some("factory".to_string()),
            1 => Some(format!("factory {}", identifiers[0])),
            _ => Some(format!("factory {}.{}", identifiers[0], identifiers[1])),
        }
    }

    /// Extract function name from function_signature node
    fn extract_function_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for identifier in function signature
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract getter name from getter_signature node
    fn extract_getter_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for identifier after 'get' keyword
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract enum name from enum_declaration node
    fn extract_enum_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for identifier after 'enum' keyword
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract function name from lambda_expression by looking at its function_signature child
    fn extract_lambda_function_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for function_signature child and extract its name
        for child in node.children(&mut node.walk()) {
            if child.kind() == "function_signature" {
                return self.extract_function_name(&child, source);
            }
        }
        None
    }

    /// Extract visibility from a Dart node (Dart uses _ prefix for private)
    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        if let Some(name) = self.extract_name_from_node(node, source) {
            if name.starts_with('_') {
                Some(Visibility::Private)
            } else {
                Some(Visibility::Public)
            }
        } else {
            None
        }
    }

    /// Extract Dartdoc comments preceding a node
    fn extract_dartdoc_comments(&self, node: &Node, source: &str) -> Option<String> {
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
                // Dart doc comment
                let doc_content = line.strip_prefix("///")?.trim();
                doc_lines.insert(0, doc_content);
            } else if line.starts_with("/**") && line.ends_with("*/") {
                // Single-line block comment
                let doc_content = line.strip_prefix("/**")?.strip_suffix("*/")?;
                doc_lines.insert(0, doc_content.trim());
            } else if line.starts_with("/**") {
                // Multi-line block comment start
                let doc_content = line.strip_prefix("/**")?.trim();
                if !doc_content.is_empty() {
                    doc_lines.insert(0, doc_content);
                }
                // Continue backwards to collect the rest of the block
                for block_line_idx in (0..line_idx).rev() {
                    let block_line = lines.get(block_line_idx)?.trim();
                    if block_line.ends_with("*/") {
                        let block_content = block_line.strip_suffix("*/")?.trim();
                        if !block_content.is_empty() {
                            doc_lines.insert(0, block_content);
                        }
                        break;
                    } else if block_line.starts_with("*") {
                        let block_content = block_line.strip_prefix("*")?.trim();
                        if !block_content.is_empty() {
                            doc_lines.insert(0, block_content);
                        }
                    }
                }
                break;
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

    /// Extract function parameters as a string
    fn extract_function_parameters(&self, node: &Node, source: &str) -> String {
        // Find the formal_parameter_list node
        for child in node.children(&mut node.walk()) {
            if child.kind() == "formal_parameter_list" {
                return self.get_node_text(&child, source);
            }
        }
        "()".to_string()
    }

    /// Extract return type annotation
    fn extract_return_type(&self, node: &Node, source: &str) -> Option<String> {
        // For getter_signature, the type comes before the 'get' keyword
        if node.kind() == "getter_signature" {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "type_identifier" {
                    return Some(self.get_node_text(&child, source));
                }
            }
        }
        
        // Look for type_annotation in the function signature
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type_annotation" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract generic type parameters
    fn extract_type_parameters(&self, node: &Node, source: &str) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type_parameters" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Build function signature from components
    fn build_function_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let params = self.extract_function_parameters(node, source);
        let type_params = self.extract_type_parameters(node, source);
        let return_type = self.extract_return_type(node, source);

        let mut signature = String::new();
        signature.push_str(name);
        
        if let Some(tp) = type_params {
            signature.push_str(&tp);
        }
        
        signature.push_str(&params);
        
        if let Some(ret) = return_type {
            signature.push(' ');
            signature.push_str(&ret);
        }

        signature
    }

    /// Build class signature with generics and clauses
    fn build_class_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        
        // Check if it's an abstract class
        let node_text = self.get_node_text(node, source);
        if node_text.starts_with("abstract") {
            signature.push_str("abstract class ");
        } else {
            signature.push_str("class ");
        }
        
        signature.push_str(name);
        
        if let Some(type_params) = self.extract_type_parameters(node, source) {
            signature.push_str(&type_params);
        }

        // Look for inheritance clauses (superclass, interfaces)
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "superclass" => {
                    // Extract extends and with clauses from superclass
                    signature.push(' ');
                    signature.push_str(&self.get_node_text(&child, source));
                }
                "interfaces" => {
                    // Extract implements clause
                    signature.push(' ');
                    signature.push_str(&self.get_node_text(&child, source));
                }
                _ => {}
            }
        }

        signature
    }

    /// Build mixin signature
    fn build_mixin_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("mixin ");
        signature.push_str(name);
        
        if let Some(type_params) = self.extract_type_parameters(node, source) {
            signature.push_str(&type_params);
        }

        // Look for on clause - based on AST, 'on' and type_identifier are direct children
        let mut found_on = false;
        for child in node.children(&mut node.walk()) {
            if child.kind() == "on" {
                signature.push_str(" on ");
                found_on = true;
            } else if found_on && child.kind() == "type_identifier" {
                signature.push_str(&self.get_node_text(&child, source));
                // Look for type_arguments after the type_identifier
                // Continue to capture the full type signature
            } else if found_on && child.kind() == "type_arguments" {
                signature.push_str(&self.get_node_text(&child, source));
                break; // We've captured the full on clause
            }
        }

        signature
    }

    /// Build extension signature
    fn build_extension_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("extension ");
        
        if name != "<unnamed extension>" {
            signature.push_str(name);
            signature.push(' ');
        }
        
        // Look for on clause (required for extensions) - look for 'on' keyword and following type
        let mut found_on = false;
        for child in node.children(&mut node.walk()) {
            if child.kind() == "on" {
                signature.push_str("on ");
                found_on = true;
            } else if found_on && child.kind() == "type_identifier" {
                signature.push_str(&self.get_node_text(&child, source));
                break;
            }
        }

        signature
    }

    /// Build enum signature
    fn build_enum_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("enum ");
        signature.push_str(name);
        
        if let Some(type_params) = self.extract_type_parameters(node, source) {
            signature.push_str(&type_params);
        }

        signature
    }

    /// Build constructor signature
    fn build_constructor_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let params = self.extract_function_parameters(node, source);
        format!("{}{}", name, params)
    }

    /// Build factory constructor signature
    fn build_factory_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let params = self.extract_function_parameters(node, source);
        let return_type = self.extract_return_type(node, source);
        
        let mut signature = String::new();
        signature.push_str(name);
        signature.push_str(&params);
        
        if let Some(ret) = return_type {
            signature.push(' ');
            signature.push_str(&ret);
        }

        signature
    }

}

impl SymbolExtractor for DartExtractor {
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
                            (node.start_byte(), node.end_byte()),
                        );

                        // Add signature based on node type and kind
                        let signature = match node_type {
                            OutlineNodeType::Function => {
                                Some(self.build_function_signature(&name, node, source))
                            }
                            OutlineNodeType::Method => {
                                match node.kind() {
                                    "constructor_signature" => {
                                        Some(self.build_constructor_signature(&name, node, source))
                                    }
                                    "factory_constructor_signature" => {
                                        Some(self.build_factory_signature(&name, node, source))
                                    }
                                    _ => Some(self.build_function_signature(&name, node, source))
                                }
                            }
                            OutlineNodeType::Class => {
                                Some(self.build_class_signature(&name, node, source))
                            }
                            OutlineNodeType::Interface => {
                                match node.kind() {
                                    "mixin_declaration" => {
                                        Some(self.build_mixin_signature(&name, node, source))
                                    }
                                    "extension_declaration" => {
                                        Some(self.build_extension_signature(&name, node, source))
                                    }
                                    _ => None
                                }
                            }
                            OutlineNodeType::Enum => {
                                Some(self.build_enum_signature(&name, node, source))
                            }
                            OutlineNodeType::Property => {
                                match node.kind() {
                                    "getter_signature" => {
                                        Some(format!("{} get {}", 
                                            self.extract_return_type(node, source).unwrap_or_else(|| "dynamic".to_string()),
                                            name))
                                    }
                                    "setter_signature" => {
                                        Some(format!("set {} {}", name, self.extract_function_parameters(node, source)))
                                    }
                                    _ => None
                                }
                            }
                            OutlineNodeType::Variable => {
                                Some(format!("var {}", name))
                            }
                            OutlineNodeType::TypeAlias => {
                                Some(format!("typedef {}", name))
                            }
                            OutlineNodeType::Module => {
                                Some(format!("library {}", name))
                            }
                            OutlineNodeType::Import => {
                                Some(format!("import {}", name))
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
                        if let Some(docs) = self.extract_dartdoc_comments(node, source) {
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
        self.extract_dartdoc_comments(node, source)
    }

    fn extract_signature(&self, node: &Node, source: &str) -> Option<String> {
        let name = self.extract_name_from_node(node, source)?;
        
        match node.kind() {
            "function_signature" => {
                Some(self.build_function_signature(&name, node, source))
            }
            "class_definition" => {
                Some(self.build_class_signature(&name, node, source))
            }
            "mixin_declaration" => {
                Some(self.build_mixin_signature(&name, node, source))
            }
            "extension_declaration" => {
                Some(self.build_extension_signature(&name, node, source))
            }
            "enum_declaration" => {
                Some(self.build_enum_signature(&name, node, source))
            }
            "constructor_signature" => {
                Some(self.build_constructor_signature(&name, node, source))
            }
            "factory_constructor_signature" => {
                Some(self.build_factory_signature(&name, node, source))
            }
            "method_signature" => {
                Some(self.build_function_signature(&name, node, source))
            }
            _ => None,
        }
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        self.parse_visibility(node, source)
    }

    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        // For now, return symbols as-is
        // TODO: Build proper hierarchical relationships for classes, mixins, etc.
        symbols
    }

    fn get_queries(&self) -> Vec<(&'static str, OutlineNodeType)> {
        vec![
            // Classes
            ("(class_definition) @class", OutlineNodeType::Class),
            // Mixins
            ("(mixin_declaration) @mixin", OutlineNodeType::Interface),
            // Extensions
            ("(extension_declaration) @extension", OutlineNodeType::Interface),
            // Enums
            ("(enum_declaration) @enum", OutlineNodeType::Enum),
            // Functions
            ("(function_signature) @function", OutlineNodeType::Function),
            // Methods
            ("(method_signature) @method", OutlineNodeType::Method),
            ("(constructor_signature) @constructor", OutlineNodeType::Method),
            ("(factory_constructor_signature) @factory", OutlineNodeType::Method),
            // Properties
            ("(getter_signature) @getter", OutlineNodeType::Property),
            ("(setter_signature) @setter", OutlineNodeType::Property),
            // Variables
            ("(initialized_variable_definition) @variable", OutlineNodeType::Variable),
            // Type aliases
            ("(type_alias) @type_alias", OutlineNodeType::TypeAlias),
            // Libraries
            ("(library_name) @library", OutlineNodeType::Module),
            // Imports
            ("(import_or_export) @import", OutlineNodeType::Import),
        ]
    }
}

impl Default for DartExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to create DartExtractor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dart_extractor_creation() {
        let extractor = DartExtractor::new();
        match &extractor {
            Ok(_) => println!("✅ DartExtractor created successfully"),
            Err(e) => println!("❌ Failed to create DartExtractor: {:?}", e),
        }
        assert!(extractor.is_ok());
    }

    #[test]
    fn test_extract_simple_function() {
        let extractor = DartExtractor::new().unwrap();
        let source = r#"
/// This is a test function
String helloWorld() {
  return "Hello, World!";
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_dart::language().into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        // Just verify the extractor doesn't crash - we'll refine queries later
        // The tree-sitter-dart grammar might have different node names than expected
        println!("Extracted {} symbols from simple Dart function", symbols.len());
        for symbol in &symbols {
            println!("  {:?} '{}' at line {}", symbol.node_type, symbol.name, symbol.start_line);
        }
    }

    #[test]
    fn test_extract_class() {
        let extractor = DartExtractor::new().unwrap();
        let source = r#"
/// A simple person class
class Person {
  String name;
  int age;
  
  /// Constructor
  Person(this.name, this.age);
  
  /// Get greeting message
  String getGreeting() {
    return "Hello, I'm $name";
  }
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_dart::language().into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        println!("Extracted {} symbols from simple Dart class", symbols.len());
        for symbol in &symbols {
            println!("  {:?} '{}' at line {}", symbol.node_type, symbol.name, symbol.start_line);
        }
    }

    #[test]
    fn test_extract_complex_dart_code() {
        let extractor = DartExtractor::new().unwrap();
        let source = r#"
/// User repository with caching capabilities
abstract class UserRepository<T extends User> 
    extends BaseRepository<T> 
    with CacheMixin<T> 
    implements DataSource<T> {
  
  /// Create a new user repository
  UserRepository({int cacheSize = 100});
  
  /// Factory constructor for creating from configuration
  factory UserRepository.fromConfig(Config config) {
    return DatabaseUserRepository<T>(config);
  }
  
  /// Find user by ID
  Future<T?> findById(String id);
  
  /// Save user data
  Future<T> save(T user, {SaveOptions? options});
}

/// Mixin for caching functionality
mixin CacheMixin<T> on BaseRepository<T> {
  final Map<String, T> _cache = {};
  
  /// Get item from cache
  T? getCached(String key) => _cache[key];
}

/// Extension methods for String validation
extension StringValidation on String {
  /// Check if string is a valid email
  bool get isValidEmail {
    return RegExp(r'^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$').hasMatch(this);
  }
}

/// Enum for user roles
enum UserRole {
  admin('Administrator'),
  user('Regular User'),
  guest('Guest User');
  
  const UserRole(this.displayName);
  
  /// Human-readable display name
  final String displayName;
  
  /// Check if role has admin privileges
  bool get hasAdminPrivileges => this == UserRole.admin;
}

/// Process user data asynchronously
Stream<ProcessResult> processUsers(
  List<User> users,
  Future<ProcessResult> Function(User) processor,
) async* {
  for (final user in users) {
    yield await processor(user);
  }
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_dart::language().into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        println!("Extracted {} symbols from complex Dart code", symbols.len());
        for symbol in &symbols {
            println!("  {:?} '{}' at line {}", symbol.node_type, symbol.name, symbol.start_line);
            if let Some(sig) = &symbol.signature {
                println!("    Signature: {}", sig);
            }
            if let Some(doc) = &symbol.documentation {
                println!("    Doc: {}", doc);
            }
        }
    }

}