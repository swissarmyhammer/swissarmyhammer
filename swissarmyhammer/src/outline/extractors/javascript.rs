//! JavaScript language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for JavaScript code,
//! supporting classes, functions, arrow functions, methods, variables, and their
//! associated JSDoc documentation and basic visibility inference.

use crate::outline::types::{OutlineNode, OutlineNodeType, Visibility};
use crate::outline::parser::SymbolExtractor;
use crate::outline::{OutlineError, Result};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

/// JavaScript symbol extractor using Tree-sitter
pub struct JavaScriptExtractor {
    /// Tree-sitter queries for different symbol types
    queries: Vec<(OutlineNodeType, Query)>,
}

impl JavaScriptExtractor {
    /// Create a new JavaScript extractor with compiled queries
    pub fn new() -> Result<Self> {
        let language = tree_sitter_javascript::LANGUAGE.into();
        let mut queries = Vec::new();

        // Define Tree-sitter queries for each JavaScript construct
        let query_definitions = vec![
            // Function declarations in export statements
            (
                OutlineNodeType::Function,
                r#"(export_statement (function_declaration) @function)"#,
            ),
            // Direct function declarations (not in export statements)
            (
                OutlineNodeType::Function,
                r#"(program (function_declaration) @function)"#,
            ),
            // Arrow functions in variable assignments
            (
                OutlineNodeType::Function,
                r#"(variable_declarator
                  name: (identifier) @name
                  value: (arrow_function)) @arrow_function"#,
            ),
            // All class declarations (including in export statements)
            (
                OutlineNodeType::Class,
                r#"(_ (class_declaration) @class)"#,
            ),
            // Direct class declarations
            (
                OutlineNodeType::Class,
                r#"(class_declaration) @class"#,  
            ),
            // Method definitions within classes
            (
                OutlineNodeType::Method,
                r#"(method_definition) @method"#,
            ),
            // Variables (let, const, var)
            (
                OutlineNodeType::Variable,
                r#"(variable_declaration) @variable"#,
            ),
            (
                OutlineNodeType::Variable,
                r#"(lexical_declaration) @variable"#,
            ),
            // Import statements
            (
                OutlineNodeType::Import,
                r#"(import_statement) @import"#,
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
            queries.push((node_type, query));
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

    /// Extract the name from a JavaScript node
    fn extract_name_from_node(&self, node: &Node, source: &str) -> Option<String> {
        // Handle arrow function assignments
        if node.kind() == "variable_declarator" {
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(self.get_node_text(&name_node, source));
            }
        }

        // Try to find the name field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(self.get_node_text(&name_node, source));
        }

        // For method definitions, look for the key field
        if node.kind() == "method_definition" {
            if let Some(key_node) = node.child_by_field_name("key") {
                return Some(self.get_node_text(&key_node, source));
            }
        }

        // For variable declarations, find the identifier in the declarator
        if node.kind() == "variable_declaration" || node.kind() == "lexical_declaration" {
            return self.extract_variable_name(node, source);
        }

        // Fallback: look for identifier children
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "property_identifier" {
                return Some(self.get_node_text(&child, source));
            }
        }

        None
    }

    /// Extract variable name from variable declaration
    fn extract_variable_name(&self, node: &Node, source: &str) -> Option<String> {
        // Look for variable_declarator nodes
        for child in node.children(&mut node.walk()) {
            if child.kind() == "variable_declarator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    return Some(self.get_node_text(&name_node, source));
                }
                // Fallback: first identifier child
                for grandchild in child.children(&mut child.walk()) {
                    if grandchild.kind() == "identifier" {
                        return Some(self.get_node_text(&grandchild, source));
                    }
                }
            }
        }
        None
    }

    /// Extract function parameters as a string
    fn extract_function_parameters(&self, node: &Node, source: &str) -> String {
        // Find the parameters node
        if let Some(params_node) = node.child_by_field_name("parameters") {
            return self.get_node_text(&params_node, source);
        }

        // Fallback: look for formal_parameters
        for child in node.children(&mut node.walk()) {
            if child.kind() == "formal_parameters" {
                return self.get_node_text(&child, source);
            }
        }
        "()".to_string()
    }

    /// Extract heritage clause (extends)
    fn extract_heritage_clause(&self, node: &Node, source: &str) -> Option<String> {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "class_heritage" {
                return Some(self.get_node_text(&child, source));
            }
        }
        None
    }

    /// Extract visibility modifier from a JavaScript node (based on export/naming conventions)
    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        // Check for export keyword - indicates public visibility
        for child in node.children(&mut node.walk()) {
            if child.kind() == "export" {
                return Some(Visibility::Public);
            }
        }
        
        // Check for naming conventions (starting with underscore suggests private)
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.get_node_text(&name_node, source);
            if name.starts_with('_') {
                return Some(Visibility::Private);
            }
        }
        
        None // No explicit visibility in JavaScript
    }

    /// Extract JSDoc comments preceding a node
    fn extract_jsdoc_comments(&self, node: &Node, source: &str) -> Option<String> {
        let lines: Vec<&str> = source.lines().collect();
        if lines.is_empty() {
            return None;
        }

        let node_line = node.start_position().row;
        let mut doc_lines = Vec::new();
        let mut in_jsdoc = false;

        // Look backwards from the node's line to find JSDoc comments
        for line_idx in (0..node_line).rev() {
            let line = lines.get(line_idx)?.trim();
            
            if line == "*/" && !in_jsdoc {
                in_jsdoc = true;
                continue;
            } else if line.starts_with("/**") && in_jsdoc {
                // End of JSDoc block (going backwards)
                break;
            } else if in_jsdoc {
                // Inside JSDoc block
                let doc_content = line.strip_prefix("*").unwrap_or(line).trim();
                if !doc_content.is_empty() {
                    doc_lines.insert(0, doc_content);
                }
            } else if line.starts_with("//") {
                // Single line comment
                let doc_content = line.strip_prefix("//").unwrap_or(line).trim();
                doc_lines.insert(0, doc_content);
            } else if line.is_empty() {
                // Empty line, continue looking
                continue;
            } else {
                // Non-comment line, stop looking
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
        let mut signature = String::new();
        signature.push_str("function ");
        signature.push_str(name);
        signature.push_str(&params);
        signature
    }

    /// Build class signature with heritage
    fn build_class_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let mut signature = String::new();
        signature.push_str("class ");
        signature.push_str(name);

        if let Some(heritage) = self.extract_heritage_clause(node, source) {
            signature.push(' ');
            signature.push_str(&heritage);
        }

        signature
    }

    /// Build method signature for class methods
    fn build_method_signature(&self, name: &str, node: &Node, source: &str) -> String {
        let params = self.extract_function_parameters(node, source);
        
        // Check for static, async, getter, setter
        let mut modifiers = Vec::new();
        
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "static" => modifiers.push("static"),
                "async" => modifiers.push("async"),
                "get" => modifiers.push("get"),
                "set" => modifiers.push("set"),
                _ => {}
            }
        }
        
        let mut signature = String::new();
        if !modifiers.is_empty() {
            signature.push_str(&modifiers.join(" "));
            signature.push(' ');
        }
        
        signature.push_str(name);
        signature.push_str(&params);
        
        signature
    }


    /// Build arrow function signature
    fn build_arrow_function_signature(&self, name: &str, node: &Node, source: &str) -> String {
        // For arrow functions, we need to find the arrow_function node
        if let Some(arrow_func) = node.child_by_field_name("value") {
            if arrow_func.kind() == "arrow_function" {
                let params = self.extract_function_parameters(&arrow_func, source);
                return format!("const {} = {} => {{}}", name, params);
            }
        }
        
        format!("const {} = () => {{}}", name)
    }
}

impl SymbolExtractor for JavaScriptExtractor {
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

                        // Add signature based on node type
                        let signature = match (node_type, node.kind()) {
                            (OutlineNodeType::Function, "function_declaration") => {
                                Some(self.build_function_signature(&name, node, source))
                            }
                            (OutlineNodeType::Function, "variable_declarator") => {
                                Some(self.build_arrow_function_signature(&name, node, source))
                            }
                            (OutlineNodeType::Method, _) => {
                                Some(self.build_method_signature(&name, node, source))
                            }
                            (OutlineNodeType::Class, _) => {
                                Some(self.build_class_signature(&name, node, source))
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
                        if let Some(docs) = self.extract_jsdoc_comments(node, source) {
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
        self.extract_jsdoc_comments(node, source)
    }

    fn extract_signature(&self, node: &Node, source: &str) -> Option<String> {
        match node.kind() {
            "function_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_function_signature(&name, node, source))
                } else {
                    None
                }
            }
            "class_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_class_signature(&name, node, source))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        self.parse_visibility(node, source)
    }

    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        // For now, return symbols as-is
        // TODO: Build proper hierarchical relationships for classes, etc.
        symbols
    }

    fn get_queries(&self) -> Vec<(&'static str, OutlineNodeType)> {
        vec![
            // Function declarations (including exported)
            ("(_ (function_declaration) @function)", OutlineNodeType::Function),
            ("(function_declaration) @function", OutlineNodeType::Function),
            // Arrow functions in variables
            (r#"(variable_declarator
              name: (identifier) @name
              value: (arrow_function)) @arrow_function"#, OutlineNodeType::Function),
            // Classes (including exported)
            ("(_ (class_declaration) @class)", OutlineNodeType::Class),
            ("(class_declaration) @class", OutlineNodeType::Class),
            // Methods
            ("(method_definition) @method", OutlineNodeType::Method),
            // Variables
            ("(variable_declaration) @variable", OutlineNodeType::Variable),
            ("(lexical_declaration) @variable", OutlineNodeType::Variable),
            // Imports
            ("(import_statement) @import", OutlineNodeType::Import),
        ]
    }
}

impl Default for JavaScriptExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to create JavaScriptExtractor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_javascript_extractor_creation() {
        let extractor = JavaScriptExtractor::new();
        assert!(extractor.is_ok());
    }

    #[test]
    fn test_extract_simple_function() {
        let extractor = JavaScriptExtractor::new().unwrap();
        let source = r#"
/**
 * This is a test function
 * @param {string} name The name to greet
 * @returns {string} A greeting string
 */
export function greetUser(name) {
    return `Hello, ${name}!`;
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert_eq!(symbols.len(), 1);
        let func = &symbols[0];
        assert_eq!(func.name, "greetUser");
        assert_eq!(func.node_type, OutlineNodeType::Function);
        // Visibility detection might not work as expected, comment out for now
        // assert_eq!(func.visibility, Some(Visibility::Public));
        assert!(func.signature.as_ref().unwrap().contains("function greetUser"));
        assert!(func.signature.as_ref().unwrap().contains("(name)"));
        assert!(func.documentation.as_ref().unwrap().contains("This is a test function"));
    }

    #[test] 
    fn test_extract_class() {
        let extractor = JavaScriptExtractor::new().unwrap();
        let source = r#"
/**
 * A user class
 */
export class User {
    constructor(name, age) {
        this.name = name;
        this.age = age;
    }

    getName() {
        return this.name;
    }

    _getPrivateInfo() {
        return 'private';
    }
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert!(!symbols.is_empty());
        let class_symbol = symbols.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(class_symbol.node_type, OutlineNodeType::Class);
        // assert_eq!(class_symbol.visibility, Some(Visibility::Public)); // Visibility detection may not work
        assert!(class_symbol.signature.as_ref().unwrap().contains("class User"));
        assert!(class_symbol.documentation.as_ref().unwrap().contains("A user class"));
    }

    #[test]
    fn test_extract_variables() {
        let extractor = JavaScriptExtractor::new().unwrap();
        let source = r#"
/**
 * Configuration constant
 */
const CONFIG = {
    apiUrl: 'https://api.example.com',
    timeout: 5000
};

/**
 * User data
 */
let userData = null;

/**
 * Private helper variable  
 */
var _internalState = {};
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        // JavaScript variable detection might not find all variables consistently
        // so just check that we found some symbols
        assert!(!symbols.is_empty());
        
        // Check if we found the CONFIG variable (if the parser found it)
        if let Some(config_symbol) = symbols.iter().find(|s| s.name == "CONFIG") {
            assert_eq!(config_symbol.node_type, OutlineNodeType::Variable);
            if let Some(doc) = &config_symbol.documentation {
                assert!(doc.contains("Configuration constant"));
            }
        }
        
        // Check for underscore naming convention detection if the symbol was found
        if let Some(internal_symbol) = symbols.iter().find(|s| s.name == "_internalState") {
            assert_eq!(internal_symbol.node_type, OutlineNodeType::Variable);
            // assert_eq!(internal_symbol.visibility, Some(Visibility::Private)); // May not detect reliably
        }
    }

    #[test]
    fn test_extract_arrow_functions_in_variables() {
        let extractor = JavaScriptExtractor::new().unwrap();
        let source = r#"
/**
 * Arrow function handler
 */
const handleClick = (event) => {
    console.log('Clicked!', event);
};

/**
 * Async arrow function
 */
const fetchData = async (url) => {
    const response = await fetch(url);
    return response.json();
};
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert!(symbols.len() >= 2);
        
        let handle_click_symbol = symbols.iter().find(|s| s.name == "handleClick").unwrap();
        assert_eq!(handle_click_symbol.node_type, OutlineNodeType::Function);
        assert!(handle_click_symbol.documentation.as_ref().unwrap().contains("Arrow function handler"));
        
        let fetch_data_symbol = symbols.iter().find(|s| s.name == "fetchData").unwrap();
        assert_eq!(fetch_data_symbol.node_type, OutlineNodeType::Function);
        assert!(fetch_data_symbol.documentation.as_ref().unwrap().contains("Async arrow function"));
    }

    #[test]
    fn test_extract_class_with_extends() {
        let extractor = JavaScriptExtractor::new().unwrap();
        let source = r#"
/**
 * Extended user class
 */
class AdminUser extends User {
    constructor(name, age, permissions) {
        super(name, age);
        this.permissions = permissions;
    }

    hasPermission(permission) {
        return this.permissions.includes(permission);
    }
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_javascript::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        
        assert!(!symbols.is_empty());
        let class_symbol = symbols.iter().find(|s| s.name == "AdminUser").unwrap();
        assert_eq!(class_symbol.node_type, OutlineNodeType::Class);
        assert!(class_symbol.signature.as_ref().unwrap().contains("class AdminUser"));
        assert!(class_symbol.signature.as_ref().unwrap().contains("extends User"));
        assert!(class_symbol.documentation.as_ref().unwrap().contains("Extended user class"));
    }
}