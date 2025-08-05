//! Python language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for Python code,  
//! supporting classes, functions, methods, properties, decorators, async functions,
//! and their associated documentation, type hints, and signature information.

use crate::outline::parser::SymbolExtractor;
use crate::outline::types::{OutlineNode, OutlineNodeType, Visibility};
use crate::outline::{OutlineError, Result};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

/// Python symbol extractor using Tree-sitter
pub struct PythonExtractor {
    /// Tree-sitter queries with their associated node types
    queries: Vec<(OutlineNodeType, Query)>,
}

impl PythonExtractor {
    /// Create a new Python extractor with compiled queries
    pub fn new() -> Result<Self> {
        let language = tree_sitter_python::LANGUAGE.into();
        let mut queries = Vec::new();

        // Define Tree-sitter queries for each Python construct
        let query_definitions = vec![
            // Function definitions (including async functions)
            (
                OutlineNodeType::Function,
                r#"(function_definition) @function"#,
            ),
            // Decorated functions (functions with decorators like @property, @classmethod)
            (
                OutlineNodeType::Function,
                r#"(decorated_definition
                  definition: (function_definition) @decorated_function)"#,
            ),
            // Class definitions
            (OutlineNodeType::Class, r#"(class_definition) @class"#),
            // Decorated classes (classes with decorators like @dataclass)
            (
                OutlineNodeType::Class,
                r#"(decorated_definition
                  definition: (class_definition) @decorated_class)"#,
            ),
            // Variable assignments at module level
            (
                OutlineNodeType::Variable,
                r#"(assignment
                  left: (identifier) @var_name) @variable"#,
            ),
            // Import statements
            (OutlineNodeType::Import, r#"(import_statement) @import"#),
            // Import from statements
            (
                OutlineNodeType::Import,
                r#"(import_from_statement) @import_from"#,
            ),
        ];

        // Compile all queries
        for (node_type, query_str) in query_definitions {
            let query = Query::new(&language, query_str).map_err(|e| {
                OutlineError::TreeSitter(format!("Failed to compile {node_type:?} query: {e}"))
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

    /// Extract the name from a Python node
    fn extract_name_from_node(&self, node: &Node, source: &str) -> Option<String> {
        // Try to find the name field first
        if let Some(name_node) = node.child_by_field_name("name") {
            return Some(self.get_node_text(&name_node, source));
        }

        // For assignments, extract the identifier
        if node.kind() == "assignment" {
            if let Some(left_node) = node.child_by_field_name("left") {
                if left_node.kind() == "identifier" {
                    return Some(self.get_node_text(&left_node, source));
                }
            }
        }

        // For decorated definitions, look at the definition
        if node.kind() == "decorated_definition" {
            if let Some(def_node) = node.child_by_field_name("definition") {
                return self.extract_name_from_node(&def_node, source);
            }
        }

        // Fallback: look for any identifier child
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "identifier" {
                    return Some(self.get_node_text(&child, source));
                }
            }
        }

        None
    }

    /// Extract decorators from a decorated definition
    fn extract_decorators(&self, node: &Node, source: &str) -> Vec<String> {
        let mut decorators = Vec::new();

        if node.kind() == "decorated_definition" {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "decorator" {
                        let decorator_text = self.get_node_text(&child, source);
                        // Remove the @ symbol and extract just the decorator name
                        if let Some(name) = decorator_text.strip_prefix('@') {
                            decorators.push(name.split('(').next().unwrap_or(name).to_string());
                        }
                    }
                }
            }
        }

        decorators
    }

    /// Extract Python function signature with type hints
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<String> {
        let node_text = self.get_node_text(node, source);

        // Find the function definition line(s) - everything up to the final ':'
        let mut signature_lines = Vec::new();

        for line in node_text.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with(':') {
                // This line ends with a colon - this should be the function definition line
                signature_lines.push(trimmed);
                break;
            } else if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                // This is a function definition line but might span multiple lines
                signature_lines.push(trimmed);
            } else if signature_lines.is_empty() {
                // Haven't found the def line yet, skip
                continue;
            } else {
                // This is a continuation of the function signature
                signature_lines.push(trimmed);
            }
        }

        if signature_lines.is_empty() {
            return None;
        }

        // Join the lines with a space
        let signature = signature_lines.join(" ");
        Some(signature)
    }

    /// Extract Python class signature with inheritance
    fn extract_class_signature(&self, node: &Node, source: &str) -> Option<String> {
        let node_text = self.get_node_text(node, source);

        // Extract the class definition line
        let first_line = node_text.lines().next()?;
        let signature = first_line.trim();

        // Clean up to include just the class declaration
        if let Some(colon_pos) = signature.find(':') {
            Some(signature[..=colon_pos].to_string())
        } else {
            Some(signature.to_string())
        }
    }

    /// Build Python function signature from components
    fn build_function_signature(&self, name: &str, node: &Node, source: &str) -> String {
        // Check if this is a decorated definition
        let parent_node = node
            .parent()
            .filter(|&parent| parent.kind() == "decorated_definition");

        let mut signature = if let Some(sig) = self.extract_function_signature(node, source) {
            sig
        } else {
            // Check if this is an async function by looking at the text
            let node_text = self.get_node_text(node, source);
            if node_text.trim_start().starts_with("async def") {
                format!("async def {name}(...):")
            } else {
                format!("def {name}(...):")
            }
        };

        // Add decorator prefix if this is a decorated function
        if let Some(parent) = parent_node {
            let decorators = self.extract_decorators(&parent, source);
            if !decorators.is_empty() {
                let decorator_prefix = decorators
                    .first()
                    .map(|d| format!("@{d} "))
                    .unwrap_or_default();
                signature = format!("{decorator_prefix}{signature}");
            }
        }

        signature
    }

    /// Build Python class signature with inheritance
    fn build_class_signature(&self, name: &str, node: &Node, source: &str) -> String {
        // Check if this is a decorated definition
        let parent_node = node
            .parent()
            .filter(|&parent| parent.kind() == "decorated_definition");

        let mut signature = if let Some(sig) = self.extract_class_signature(node, source) {
            sig
        } else {
            format!("class {name}:")
        };

        // Add decorator prefix if this is a decorated class (e.g., @dataclass)
        if let Some(parent) = parent_node {
            let decorators = self.extract_decorators(&parent, source);
            if !decorators.is_empty() {
                let decorator_prefix = decorators
                    .first()
                    .map(|d| format!("@{d} "))
                    .unwrap_or_default();
                signature = format!("{decorator_prefix}{signature}");
            }
        }

        signature
    }

    /// Generate signature for import statements
    fn build_import_signature(&self, node: &Node, source: &str) -> String {
        let node_text = self.get_node_text(node, source);
        let first_line = node_text.lines().next().unwrap_or("").trim();
        first_line.to_string()
    }

    /// Generate signature for variable assignments
    fn build_variable_signature(&self, node: &Node, source: &str) -> String {
        let node_text = self.get_node_text(node, source);
        let first_line = node_text.lines().next().unwrap_or("").trim();
        first_line.to_string()
    }

    /// Extract docstring from a function or class node
    fn extract_docstring(&self, node: &Node, source: &str) -> Option<String> {
        // Look for a string literal as the first statement in the body
        if let Some(body) = node.child_by_field_name("body") {
            for i in 0..body.child_count() {
                if let Some(child) = body.child(i) {
                    if child.kind() == "expression_statement" {
                        // Check if this expression statement contains a string
                        for j in 0..child.child_count() {
                            if let Some(expr_child) = child.child(j) {
                                if expr_child.kind() == "string" {
                                    let docstring = self.get_node_text(&expr_child, source);
                                    // Clean up the docstring by removing quotes and unnecessary whitespace
                                    return Some(self.clean_docstring(&docstring));
                                }
                            }
                        }
                    }
                    // Only check the first statement
                    break;
                }
            }
        }
        None
    }

    /// Clean up a docstring by removing quotes and formatting
    fn clean_docstring(&self, docstring: &str) -> String {
        let mut cleaned = docstring.trim();

        // Remove triple quotes
        if (cleaned.starts_with("\"\"\"") && cleaned.ends_with("\"\"\""))
            || (cleaned.starts_with("'''") && cleaned.ends_with("'''"))
        {
            cleaned = &cleaned[3..cleaned.len() - 3];
        } else if (cleaned.starts_with('"') && cleaned.ends_with('"'))
            || (cleaned.starts_with('\'') && cleaned.ends_with('\''))
        {
            cleaned = &cleaned[1..cleaned.len() - 1];
        }

        // Clean up whitespace and return first line or first sentence
        let cleaned = cleaned.trim();
        let first_line = cleaned.lines().next().unwrap_or("").trim();

        // If the first line ends with a period, use just that
        if first_line.ends_with('.') {
            first_line.to_string()
        } else {
            // Otherwise, take up to the first sentence
            if let Some(period_pos) = cleaned.find(". ") {
                cleaned[..=period_pos].to_string()
            } else {
                first_line.to_string()
            }
        }
    }

    /// Determine visibility for Python symbols (public vs private)
    fn get_visibility(&self, name: &str) -> Option<Visibility> {
        if name.starts_with('_') {
            if name.starts_with("__") && name.ends_with("__") {
                // Magic methods are public
                Some(Visibility::Public)
            } else {
                // Private or protected
                Some(Visibility::Private)
            }
        } else {
            Some(Visibility::Public)
        }
    }
}

impl SymbolExtractor for PythonExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Result<Vec<OutlineNode>> {
        let mut symbols = Vec::new();
        let root_node = tree.root_node();

        // Process each query
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
                        let signature = match node_type {
                            OutlineNodeType::Function => {
                                Some(self.build_function_signature(&name, node, source))
                            }
                            OutlineNodeType::Class => {
                                Some(self.build_class_signature(&name, node, source))
                            }
                            OutlineNodeType::Import => {
                                Some(self.build_import_signature(node, source))
                            }
                            OutlineNodeType::Variable => {
                                Some(self.build_variable_signature(node, source))
                            }
                            _ => None,
                        };

                        if let Some(sig) = signature {
                            outline_node = outline_node.with_signature(sig);
                        }

                        // Add visibility
                        if let Some(visibility) = self.get_visibility(&name) {
                            outline_node = outline_node.with_visibility(visibility);
                        }

                        // Add documentation
                        if let Some(docs) = self.extract_docstring(node, source) {
                            outline_node = outline_node.with_documentation(docs);
                        }

                        symbols.push(outline_node);
                    }
                }
            }
        }

        // Sort symbols by line number
        symbols.sort_by_key(|s| s.start_line);

        Ok(symbols)
    }

    fn extract_documentation(&self, node: &Node, source: &str) -> Option<String> {
        self.extract_docstring(node, source)
    }

    fn extract_signature(&self, node: &Node, source: &str) -> Option<String> {
        if let Some(name) = self.extract_name_from_node(node, source) {
            match node.kind() {
                "function_definition" => Some(self.build_function_signature(&name, node, source)),
                "class_definition" => Some(self.build_class_signature(&name, node, source)),
                "import_statement" | "import_from_statement" => {
                    Some(self.build_import_signature(node, source))
                }
                "assignment" => Some(self.build_variable_signature(node, source)),
                _ => None,
            }
        } else {
            None
        }
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        if let Some(name) = self.extract_name_from_node(node, source) {
            self.get_visibility(&name)
        } else {
            None
        }
    }

    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        // For now, return symbols as-is
        // TODO: Build proper hierarchical relationships for classes and their methods
        symbols
    }

    fn get_queries(&self) -> Vec<(&'static str, OutlineNodeType)> {
        vec![
            // Functions
            ("(function_definition) @function", OutlineNodeType::Function),
            // Classes
            ("(class_definition) @class", OutlineNodeType::Class),
            // Variables
            (
                "(assignment left: (identifier) @var_name) @variable",
                OutlineNodeType::Variable,
            ),
            // Imports
            ("(import_statement) @import", OutlineNodeType::Import),
            (
                "(import_from_statement) @import_from",
                OutlineNodeType::Import,
            ),
        ]
    }
}

impl Default for PythonExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to create PythonExtractor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_extractor_creation() {
        let extractor = PythonExtractor::new();
        assert!(extractor.is_ok());
    }

    #[test]
    fn test_extract_simple_function() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
def hello_world() -> str:
    """This is a test function."""
    return "Hello, World!"
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        assert_eq!(symbols.len(), 1);
        let func = &symbols[0];
        assert_eq!(func.name, "hello_world");
        assert_eq!(func.node_type, OutlineNodeType::Function);
        assert_eq!(func.visibility, Some(Visibility::Public));
        assert!(func
            .signature
            .as_ref()
            .unwrap()
            .contains("def hello_world() -> str:"));
        assert_eq!(
            func.documentation.as_ref().unwrap(),
            "This is a test function."
        );
    }

    #[test]
    fn test_extract_async_function() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
async def fetch_data(url: str) -> dict:
    """Fetch data from URL."""
    return {}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        assert_eq!(symbols.len(), 1);
        let func = &symbols[0];
        assert_eq!(func.name, "fetch_data");
        assert_eq!(func.node_type, OutlineNodeType::Function);
        assert!(func
            .signature
            .as_ref()
            .unwrap()
            .contains("async def fetch_data(url: str) -> dict:"));
    }

    #[test]
    fn test_extract_class() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
class Person:
    """A simple person class."""
    
    def __init__(self, name: str):
        self.name = name
    
    def greet(self) -> str:
        return f"Hello, I'm {self.name}"
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        // Should find the class
        let classes: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Class)
            .collect();
        assert_eq!(classes.len(), 1);

        let class = classes[0];
        assert_eq!(class.name, "Person");
        assert!(class.signature.as_ref().unwrap().contains("class Person:"));
        assert_eq!(
            class.documentation.as_ref().unwrap(),
            "A simple person class."
        );
    }

    #[test]
    fn test_extract_private_methods() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
class TestClass:
    def public_method(self):
        pass
    
    def _private_method(self):
        pass
    
    def __dunder_method__(self):
        pass
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        // Check visibility detection
        let public_symbols: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| s.visibility == Some(Visibility::Public))
            .collect();
        let private_symbols: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| s.visibility == Some(Visibility::Private))
            .collect();

        // Should have public class and dunder method as public, private method as private
        assert!(!public_symbols.is_empty());
        assert!(!private_symbols.is_empty());
    }

    #[test]
    fn test_extract_imports() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
import os
from typing import List, Dict
from collections import defaultdict
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        let imports: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Import)
            .collect();
        assert!(!imports.is_empty());
    }

    #[test]
    fn test_extract_variables() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
VERSION = "1.0.0"
DEBUG = True
CONFIG = {
    "host": "localhost",
    "port": 8080
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        let variables: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Variable)
            .collect();
        assert!(!variables.is_empty());

        let names: Vec<&String> = variables.iter().map(|s| &s.name).collect();
        assert!(names.contains(&&"VERSION".to_string()));
        assert!(names.contains(&&"DEBUG".to_string()));
        assert!(names.contains(&&"CONFIG".to_string()));
    }

    #[test]
    fn test_extract_decorated_functions_and_classes() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
@dataclass
class User:
    """User data model."""
    name: str
    email: str
    
    @property
    def display_name(self) -> str:
        """Get formatted display name."""
        return f"{self.name} ({self.email})"
    
    @classmethod
    def from_dict(cls, data: dict) -> 'User':
        """Create user from dictionary."""
        return cls(data['name'], data['email'])
    
    @staticmethod
    def validate_email(email: str) -> bool:
        """Validate email format."""
        return '@' in email

@pytest.fixture
def sample_user():
    """Sample user for testing."""
    return User("John", "john@example.com")
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        println!(
            "Successfully extracted {} symbols from decorated Python code",
            symbols.len()
        );
        for symbol in &symbols {
            println!(
                "  {:?} '{}' at line {}",
                symbol.node_type, symbol.name, symbol.start_line
            );
            if let Some(sig) = &symbol.signature {
                println!("    Signature: {sig}");
            }
            if let Some(doc) = &symbol.documentation {
                println!("    Doc: {doc}");
            }
        }

        // Should find decorated class
        let decorated_classes: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| {
                s.node_type == OutlineNodeType::Class
                    && s.signature
                        .as_ref()
                        .is_some_and(|sig| sig.contains("@dataclass"))
            })
            .collect();
        assert!(
            !decorated_classes.is_empty(),
            "Should find @dataclass decorated class"
        );

        // Should find property methods
        let property_methods: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| {
                s.signature
                    .as_ref()
                    .is_some_and(|sig| sig.contains("@property"))
            })
            .collect();
        assert!(
            !property_methods.is_empty(),
            "Should find @property decorated methods"
        );

        // Should find classmethod
        let class_methods: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| {
                s.signature
                    .as_ref()
                    .is_some_and(|sig| sig.contains("@classmethod"))
            })
            .collect();
        assert!(
            !class_methods.is_empty(),
            "Should find @classmethod decorated methods"
        );

        // Should find staticmethod
        let static_methods: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| {
                s.signature
                    .as_ref()
                    .is_some_and(|sig| sig.contains("@staticmethod"))
            })
            .collect();
        assert!(
            !static_methods.is_empty(),
            "Should find @staticmethod decorated methods"
        );
    }

    #[test]
    fn test_extract_complex_python_code() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
"""User management module.

This module provides classes and functions for managing user data
and authentication within the application.
"""

from typing import Optional, List, Dict, Any, Protocol
from dataclasses import dataclass, field
import asyncio

@dataclass
class User:
    """User data model."""
    id: str
    name: str
    email: str
    permissions: List[str] = field(default_factory=list)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'User':
        """Create user from dictionary data."""
        return cls(
            id=data['id'],
            name=data['name'],
            email=data.get('email', ''),
            permissions=data.get('permissions', [])
        )
    
    @property
    def display_name(self) -> str:
        """Get formatted display name."""
        return f"{self.name} ({self.email})"
    
    def __str__(self) -> str:
        return self.display_name

class Repository:
    """Base repository class."""
    
    def __init__(self, connection_string: str):
        self.connection_string = connection_string
    
    async def find_by_id(self, id: str) -> Optional[dict]:
        """Find entity by ID."""
        pass

async def process_users(users: List[User]) -> List[User]:
    """Process list of users."""
    return users

def create_user_factory(default_permissions: List[str]):
    """Create a factory function for users."""
    def factory(id: str, name: str, email: str = '') -> User:
        return User(
            id=id,
            name=name, 
            email=email,
            permissions=default_permissions[:]
        )
    return factory

VERSION = "1.0.0"
DEFAULT_PERMISSIONS = ["read", "write"]
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        // Should extract multiple symbol types
        assert!(!symbols.is_empty());

        // Check that we got various types
        let types: std::collections::HashSet<&OutlineNodeType> =
            symbols.iter().map(|s| &s.node_type).collect();

        assert!(types.contains(&OutlineNodeType::Class));
        assert!(types.contains(&OutlineNodeType::Function));
        assert!(types.contains(&OutlineNodeType::Import));
        assert!(types.contains(&OutlineNodeType::Variable));

        // Check specific symbols exist with correct names
        let names: Vec<&String> = symbols.iter().map(|s| &s.name).collect();
        assert!(names.contains(&&"User".to_string()));
        assert!(names.contains(&&"Repository".to_string()));
        assert!(names.contains(&&"process_users".to_string()));
        assert!(names.contains(&&"create_user_factory".to_string()));
        assert!(names.contains(&&"VERSION".to_string()));
        assert!(names.contains(&&"DEFAULT_PERMISSIONS".to_string()));

        // Check that some documentation was extracted
        let has_docs = symbols.iter().any(|s| s.documentation.is_some());
        assert!(has_docs, "Should find symbols with documentation");

        // Check that async functions are detected
        let async_functions: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| {
                s.signature
                    .as_ref()
                    .is_some_and(|sig| sig.contains("async def"))
            })
            .collect();
        assert!(!async_functions.is_empty(), "Should find async functions");

        println!(
            "Successfully extracted {} symbols from complex Python code",
            symbols.len()
        );
        for symbol in &symbols {
            println!(
                "  {:?} '{}' at line {}",
                symbol.node_type, symbol.name, symbol.start_line
            );
        }
    }
}
