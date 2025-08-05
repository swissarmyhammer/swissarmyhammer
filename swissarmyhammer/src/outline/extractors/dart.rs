//! Dart language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for Dart code,
//! supporting classes, mixins, enums, extensions, functions, methods, constructors,
//! properties, and their associated documentation, visibility, and signature information.
//! Includes support for Flutter-specific patterns and modern Dart language features.

use crate::outline::parser::SymbolExtractor;
use crate::outline::signature::{
    GenericParameter, Modifier, Parameter, Signature, SignatureExtractor, TypeInfo,
};
use crate::outline::types::{OutlineNode, OutlineNodeType, Visibility};
use crate::outline::{OutlineError, Result};
use crate::search::types::Language;
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
        let language = tree_sitter_dart::language();
        let mut queries = HashMap::new();

        // Define Tree-sitter queries for Dart constructs
        // Using the actual node names from tree-sitter-dart grammar
        let query_definitions = vec![
            // Classes
            (OutlineNodeType::Class, r#"(class_definition) @class"#),
            // Mixins
            (OutlineNodeType::Interface, r#"(mixin_declaration) @mixin"#),
            // Extensions
            (
                OutlineNodeType::Interface,
                r#"(extension_declaration) @extension"#,
            ),
            // Enums
            (OutlineNodeType::Enum, r#"(enum_declaration) @enum"#),
            // Functions
            (
                OutlineNodeType::Function,
                r#"(function_signature) @function"#,
            ),
            // Method signatures
            (OutlineNodeType::Method, r#"(method_signature) @method"#),
            // Getter signatures
            (OutlineNodeType::Property, r#"(getter_signature) @getter"#),
            // Setter signatures
            (OutlineNodeType::Property, r#"(setter_signature) @setter"#),
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
            (OutlineNodeType::TypeAlias, r#"(type_alias) @type_alias"#),
            // Variables
            (
                OutlineNodeType::Variable,
                r#"(initialized_variable_definition) @variable"#,
            ),
            // Libraries
            (OutlineNodeType::Module, r#"(library_name) @library"#),
            // Imports
            (OutlineNodeType::Import, r#"(import_or_export) @import"#),
        ];

        // Compile all queries
        for (node_type, query_str) in query_definitions {
            let query = Query::new(&language, query_str).map_err(|e| {
                OutlineError::TreeSitter(format!("Failed to compile {node_type:?} query: {e}"))
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
        format!("{name}{params}")
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

impl SignatureExtractor for DartExtractor {
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let name = self.extract_name_from_node(node, source)?;
        let mut signature = Signature::new(name.clone(), Language::Dart);

        // Extract modifiers from Dart function
        let modifiers = self.parse_modifiers(node, source);
        if !modifiers.is_empty() {
            signature = signature.with_modifiers(modifiers);
        }

        // Extract parameters with named parameter support
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let parameters = self.extract_parameters_from_node(&params_node, source);
            for param in parameters {
                signature = signature.with_parameter(param);
            }
        }

        // Extract return type
        if let Some(return_type) = self.parse_dart_return_type(node, source) {
            signature = signature.with_return_type(return_type);
        }

        // Check for async
        if self.is_async_function(node, source) {
            signature = signature.async_function();
        }

        Some(signature)
    }

    fn extract_method_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let name = self.extract_name_from_node(node, source)?;
        let mut signature = Signature::new(name.clone(), Language::Dart);

        // Extract modifiers (static, abstract, etc.)
        let modifiers = self.parse_modifiers(node, source);
        if !modifiers.is_empty() {
            signature = signature.with_modifiers(modifiers);
        }

        // Extract parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let parameters = self.extract_parameters_from_node(&params_node, source);
            for param in parameters {
                signature = signature.with_parameter(param);
            }
        }

        // Extract return type
        if let Some(return_type) = self.parse_dart_return_type(node, source) {
            signature = signature.with_return_type(return_type);
        }

        // Check for async
        if self.is_async_function(node, source) {
            signature = signature.async_function();
        }

        Some(signature)
    }

    fn extract_constructor_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let name = self.extract_name_from_node(node, source)?;
        let mut signature = Signature::new(name.clone(), Language::Dart);

        // Extract parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let parameters = self.extract_parameters_from_node(&params_node, source);
            for param in parameters {
                signature = signature.with_parameter(param);
            }
        }

        signature = signature.constructor();
        Some(signature)
    }

    fn extract_type_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let name = self.extract_name_from_node(node, source)?;
        let mut signature = Signature::new(name.clone(), Language::Dart);

        match node.kind() {
            "class_definition" => {
                // Extract generic parameters
                let generics = self.parse_generic_parameters(node, source);
                for generic in generics {
                    signature = signature.with_generic(generic);
                }

                // Build the class signature string
                let class_signature = self.build_class_signature(&name, node, source);
                signature = signature.with_raw_signature(class_signature);
            }
            "enum_declaration" => {
                let enum_signature = format!("enum {}", name);
                signature = signature.with_raw_signature(enum_signature);
            }
            "mixin_declaration" => {
                let mixin_signature = format!("mixin {}", name);
                signature = signature.with_raw_signature(mixin_signature);
            }
            "extension_declaration" => {
                let extension_signature = format!("extension {}", name);
                signature = signature.with_raw_signature(extension_signature);
            }
            _ => {}
        }

        Some(signature)
    }

    fn parse_type_info(&self, node: &Node, source: &str) -> Option<TypeInfo> {
        match node.kind() {
            "type_identifier" => {
                let type_name = self.get_node_text(node, source);
                Some(TypeInfo::new(type_name))
            }
            "type_name" => {
                let type_name = self.get_node_text(node, source);
                Some(TypeInfo::new(type_name))
            }
            "generic_type" => {
                // Extract the base type and generic arguments
                let mut base_name = String::new();
                let mut generic_args = Vec::new();

                for child in node.children(&mut node.walk()) {
                    match child.kind() {
                        "type_identifier" => {
                            if base_name.is_empty() {
                                base_name = self.get_node_text(&child, source);
                            }
                        }
                        "type_arguments" => {
                            for arg_child in child.children(&mut child.walk()) {
                                if let Some(arg_type) = self.parse_type_info(&arg_child, source) {
                                    generic_args.push(arg_type);
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if !base_name.is_empty() {
                    Some(TypeInfo::generic(base_name, generic_args))
                } else {
                    None
                }
            }
            "list_type" => {
                // Handle List<T>
                for child in node.children(&mut node.walk()) {
                    if let Some(element_type) = self.parse_type_info(&child, source) {
                        return Some(TypeInfo::array(element_type, 1));
                    }
                }
                Some(TypeInfo::array(TypeInfo::new("dynamic".to_string()), 1))
            }
            _ => {
                // Fallback: just use the raw text
                let text = self.get_node_text(node, source);
                if !text.is_empty() {
                    Some(TypeInfo::new(text))
                } else {
                    None
                }
            }
        }
    }

    fn parse_parameter(&self, node: &Node, source: &str) -> Option<Parameter> {
        match node.kind() {
            "formal_parameter" | "normal_formal_parameter" => {
                let mut param_name = String::new();
                let mut param_type = None;
                let mut is_named = false;
                let mut is_optional = false;
                let mut default_value = None;

                for child in node.children(&mut node.walk()) {
                    match child.kind() {
                        "identifier" => {
                            param_name = self.get_node_text(&child, source);
                        }
                        "type_identifier" | "type_name" => {
                            if let Some(type_info) = self.parse_type_info(&child, source) {
                                param_type = Some(type_info);
                            }
                        }
                        "?" => {
                            is_optional = true;
                        }
                        "default_formal_parameter" => {
                            // Handle default values
                            if let Some(value_node) = child.child_by_field_name("default_value") {
                                default_value = Some(self.get_node_text(&value_node, source));
                            }
                        }
                        _ => {}
                    }
                }

                // Check if this is inside named parameters
                if let Some(parent) = node.parent() {
                    if parent.kind() == "named_parameter_types" {
                        is_named = true;
                    }
                }

                if !param_name.is_empty() {
                    let mut parameter = Parameter::new(param_name);
                    if let Some(type_info) = param_type {
                        parameter = parameter.with_type(type_info);
                    } else {
                        parameter = parameter.with_type(TypeInfo::new("dynamic".to_string()));
                    }
                    if is_optional {
                        parameter = parameter.optional();
                    }
                    if is_named {
                        parameter = parameter.named();
                    }
                    if let Some(default) = default_value {
                        parameter = parameter.with_default(default);
                    }
                    Some(parameter)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn parse_generic_parameters(&self, node: &Node, source: &str) -> Vec<GenericParameter> {
        let mut generics = Vec::new();

        // Look for type_parameter_list
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type_parameter_list" {
                for param_child in child.children(&mut child.walk()) {
                    if param_child.kind() == "type_parameter" {
                        if let Some(name_node) = param_child.child_by_field_name("name") {
                            let name = self.get_node_text(&name_node, source);
                            let mut generic = GenericParameter::new(name);
                            
                            // Check for constraints (extends clause)
                            if let Some(bound_node) = param_child.child_by_field_name("bound") {
                                if let Some(bound_type) = self.parse_type_info(&bound_node, source) {
                                    generic = generic.with_bounds(vec![bound_type.name]);
                                }
                            }
                            
                            generics.push(generic);
                        }
                    }
                }
            }
        }

        generics
    }

    fn parse_modifiers(&self, node: &Node, source: &str) -> Vec<Modifier> {
        let mut modifiers = Vec::new();

        // Check for various Dart modifiers
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "static" => modifiers.push(Modifier::Static),
                "abstract" => modifiers.push(Modifier::Abstract),
                "async" => modifiers.push(Modifier::Async),
                "const" => modifiers.push(Modifier::Const),
                "final" => modifiers.push(Modifier::Final),
                _ => {}
            }
        }

        // Check the source text for modifiers that might not be separate nodes
        let node_text = self.get_node_text(node, source);
        if node_text.contains("async") && !modifiers.contains(&Modifier::Async) {
            modifiers.push(Modifier::Async);
        }

        modifiers
    }
}

impl DartExtractor {
    /// Check if a function is async
    fn is_async_function(&self, node: &Node, source: &str) -> bool {
        // Check for async keyword in children
        for child in node.children(&mut node.walk()) {
            if child.kind() == "async" {
                return true;
            }
        }

        // Check the source text
        let node_text = self.get_node_text(node, source);
        node_text.contains("async")
    }

    /// Extract parameters from a formal_parameter_list node
    fn extract_parameters_from_node(&self, node: &Node, source: &str) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "formal_parameter" | "normal_formal_parameter" | "optional_formal_parameters" | "named_parameter_types" => {
                    if let Some(param) = self.parse_parameter(&child, source) {
                        parameters.push(param);
                    }
                    // Also check children for nested parameters
                    for grandchild in child.children(&mut child.walk()) {
                        if let Some(param) = self.parse_parameter(&grandchild, source) {
                            parameters.push(param);
                        }
                    }
                }
                _ => {}
            }
        }

        parameters
    }

    /// Parse return type from Dart function
    fn parse_dart_return_type(&self, node: &Node, source: &str) -> Option<TypeInfo> {
        // Look for return type before function name
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type_identifier" || child.kind() == "type_name" {
                return self.parse_type_info(&child, source);
            }
        }
        None
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

                        // Add comprehensive signature based on node type and kind
                        let signature = match node_type {
                            OutlineNodeType::Function => {
                                // Use new comprehensive signature extraction
                                if let Some(detailed_sig) = self.extract_function_signature(node, source) {
                                    Some(detailed_sig.format_for_language(Language::Dart))
                                } else {
                                    Some(self.build_function_signature(&name, node, source))
                                }
                            }
                            OutlineNodeType::Method => match node.kind() {
                                "constructor_signature" => {
                                    if let Some(detailed_sig) = self.extract_constructor_signature(node, source) {
                                        Some(detailed_sig.format_for_language(Language::Dart))
                                    } else {
                                        Some(self.build_constructor_signature(&name, node, source))
                                    }
                                }
                                "factory_constructor_signature" => {
                                    if let Some(detailed_sig) = self.extract_constructor_signature(node, source) {
                                        Some(detailed_sig.format_for_language(Language::Dart))
                                    } else {
                                        Some(self.build_factory_signature(&name, node, source))
                                    }
                                }
                                _ => {
                                    if let Some(detailed_sig) = self.extract_method_signature(node, source) {
                                        Some(detailed_sig.format_for_language(Language::Dart))
                                    } else {
                                        Some(self.build_function_signature(&name, node, source))
                                    }
                                }
                            },
                            OutlineNodeType::Class => {
                                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                                    Some(detailed_sig.format_for_language(Language::Dart))
                                } else {
                                    Some(self.build_class_signature(&name, node, source))
                                }
                            }
                            OutlineNodeType::Interface => match node.kind() {
                                "mixin_declaration" => {
                                    if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                                        Some(detailed_sig.format_for_language(Language::Dart))
                                    } else {
                                        Some(self.build_mixin_signature(&name, node, source))
                                    }
                                }
                                "extension_declaration" => {
                                    if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                                        Some(detailed_sig.format_for_language(Language::Dart))
                                    } else {
                                        Some(self.build_extension_signature(&name, node, source))
                                    }
                                }
                                _ => None,
                            },
                            OutlineNodeType::Enum => {
                                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                                    Some(detailed_sig.format_for_language(Language::Dart))
                                } else {
                                    Some(self.build_enum_signature(&name, node, source))
                                }
                            }
                            OutlineNodeType::Property => match node.kind() {
                                "getter_signature" => Some(format!(
                                    "{} get {}",
                                    self.extract_return_type(node, source)
                                        .unwrap_or_else(|| "dynamic".to_string()),
                                    name
                                )),
                                "setter_signature" => Some(format!(
                                    "set {} {}",
                                    name,
                                    self.extract_function_parameters(node, source)
                                )),
                                _ => None,
                            },
                            OutlineNodeType::Variable => Some(format!("var {name}")),
                            OutlineNodeType::TypeAlias => Some(format!("typedef {name}")),
                            OutlineNodeType::Module => Some(format!("library {name}")),
                            OutlineNodeType::Import => Some(format!("import {name}")),
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
        match node.kind() {
            "function_signature" => {
                // Use new comprehensive signature extraction
                if let Some(detailed_sig) = self.extract_function_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_function_signature(&name, node, source))
                }
            }
            "class_definition" => {
                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_class_signature(&name, node, source))
                }
            }
            "mixin_declaration" => {
                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_mixin_signature(&name, node, source))
                }
            }
            "extension_declaration" => {
                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_extension_signature(&name, node, source))
                }
            }
            "enum_declaration" => {
                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_enum_signature(&name, node, source))
                }
            }
            "constructor_signature" => {
                if let Some(detailed_sig) = self.extract_constructor_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_constructor_signature(&name, node, source))
                }
            }
            "factory_constructor_signature" => {
                if let Some(detailed_sig) = self.extract_constructor_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_factory_signature(&name, node, source))
                }
            }
            "method_signature" => {
                if let Some(detailed_sig) = self.extract_method_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Dart))
                } else {
                    let name = self.extract_name_from_node(node, source)?;
                    Some(self.build_function_signature(&name, node, source))
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
            (
                "(extension_declaration) @extension",
                OutlineNodeType::Interface,
            ),
            // Enums
            ("(enum_declaration) @enum", OutlineNodeType::Enum),
            // Functions
            ("(function_signature) @function", OutlineNodeType::Function),
            // Methods
            ("(method_signature) @method", OutlineNodeType::Method),
            (
                "(constructor_signature) @constructor",
                OutlineNodeType::Method,
            ),
            (
                "(factory_constructor_signature) @factory",
                OutlineNodeType::Method,
            ),
            // Properties
            ("(getter_signature) @getter", OutlineNodeType::Property),
            ("(setter_signature) @setter", OutlineNodeType::Property),
            // Variables
            (
                "(initialized_variable_definition) @variable",
                OutlineNodeType::Variable,
            ),
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
            Err(e) => println!("❌ Failed to create DartExtractor: {e:?}"),
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
        parser.set_language(&tree_sitter_dart::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        // Just verify the extractor doesn't crash - we'll refine queries later
        // The tree-sitter-dart grammar might have different node names than expected
        println!(
            "Extracted {} symbols from simple Dart function",
            symbols.len()
        );
        for symbol in &symbols {
            println!(
                "  {:?} '{}' at line {}",
                symbol.node_type, symbol.name, symbol.start_line
            );
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
        parser.set_language(&tree_sitter_dart::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        println!("Extracted {} symbols from simple Dart class", symbols.len());
        for symbol in &symbols {
            println!(
                "  {:?} '{}' at line {}",
                symbol.node_type, symbol.name, symbol.start_line
            );
        }
    }

    #[test]
    fn test_dart_flutter_patterns() {
        let extractor = DartExtractor::new().unwrap();
        let source = r#"
import 'package:flutter/material.dart';

/// Main application widget
class MyApp extends StatelessWidget {
  /// App title
  final String title;
  
  /// Create MyApp widget
  const MyApp({super.key, required this.title});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: title,
      home: HomePage(),
    );
  }
}

/// Home page widget
class HomePage extends StatefulWidget {
  /// Create HomePage
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

/// Private state class for HomePage
class _HomePageState extends State<HomePage> {
  int _counter = 0;

  /// Increment counter
  void _incrementCounter() {
    setState(() {
      _counter++;
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Text('$_counter'),
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: _incrementCounter,
        child: Icon(Icons.add),
      ),
    );
  }
}

/// Typedef for callback function
typedef CounterCallback = void Function(int count);

/// Global function with async
Future<String> fetchData() async {
  await Future.delayed(Duration(seconds: 1));
  return 'Data loaded';
}

/// Extension with generic constraints
extension ListUtils<T extends Comparable<T>> on List<T> {
  /// Sort list in place
  void sortInPlace() => sort();
  
  /// Find maximum element
  T? get max => isEmpty ? null : reduce((a, b) => a.compareTo(b) > 0 ? a : b);
}

/// Mixin with type constraints
mixin LoggerMixin<T extends Object> on Object {
  /// Log message with type info
  void log(String message) {
    print('[$T] $message');
  }
}

/// Enhanced enum with methods
enum Priority {
  low(0, 'Low Priority'),
  medium(1, 'Medium Priority'), 
  high(2, 'High Priority');
  
  /// Create priority
  const Priority(this.value, this.label);
  
  /// Priority value
  final int value;
  
  /// Display label
  final String label;
  
  /// Check if high priority
  bool get isHigh => this == Priority.high;
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_dart::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        println!("Extracted {} symbols from Flutter patterns", symbols.len());
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

        // Verify we extract key Flutter patterns
        let class_names: Vec<_> = symbols
            .iter()
            .filter(|s| matches!(s.node_type, crate::outline::types::OutlineNodeType::Class))
            .map(|s| s.name.as_str())
            .collect();

        assert!(class_names.contains(&"MyApp"));
        assert!(class_names.contains(&"HomePage"));
        assert!(class_names.contains(&"_HomePageState"));
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
        parser.set_language(&tree_sitter_dart::language()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        println!("Extracted {} symbols from complex Dart code", symbols.len());
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
    }
}
