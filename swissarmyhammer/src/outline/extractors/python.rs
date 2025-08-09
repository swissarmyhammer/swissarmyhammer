//! Python language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for Python code,  
//! supporting classes, functions, methods, properties, decorators, async functions,
//! and their associated documentation, type hints, and signature information.

use crate::outline::parser::SymbolExtractor;
use crate::outline::signature::{
    GenericParameter, Modifier, Parameter, Signature, SignatureExtractor, TypeInfo,
};
use crate::outline::types::{OutlineNode, OutlineNodeType, Visibility};
use crate::outline::{OutlineError, Result};
use crate::search::types::Language;
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
        // Note: Using more comprehensive queries to avoid duplicates between decorated and non-decorated symbols
        let query_definitions = vec![
            // Function definitions (both decorated and non-decorated)
            (
                OutlineNodeType::Function,
                r#"[(function_definition) @function
                   (decorated_definition definition: (function_definition)) @function]"#,
            ),
            // Class definitions (both decorated and non-decorated)
            (
                OutlineNodeType::Class,
                r#"[(class_definition) @class
                   (decorated_definition definition: (class_definition)) @class]"#,
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

impl SignatureExtractor for PythonExtractor {
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let name = self.extract_name_from_node(node, source)?;
        let mut signature = Signature::new(name.clone(), Language::Python);

        // Extract modifiers from Python function
        let modifiers = self.parse_modifiers(node, source);
        if !modifiers.is_empty() {
            signature = signature.with_modifiers(modifiers);
        }

        // Extract parameters with Python-specific features (*args, **kwargs, type hints)
        if let Some(params_node) = node.child_by_field_name("parameters") {
            let parameters = self.extract_parameters_from_node(&params_node, source);
            for param in parameters {
                signature = signature.with_parameter(param);
            }
        }

        // Extract return type annotation
        if let Some(return_type) = self.parse_python_return_type(node, source) {
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
        let mut signature = Signature::new(name.clone(), Language::Python);

        // Extract modifiers (static, class method, etc.)
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

        // Extract return type annotation
        if let Some(return_type) = self.parse_python_return_type(node, source) {
            signature = signature.with_return_type(return_type);
        }

        // Check for async
        if self.is_async_function(node, source) {
            signature = signature.async_function();
        }

        Some(signature)
    }

    fn extract_constructor_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let mut signature = Signature::new("__init__".to_string(), Language::Python);

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
        let mut signature = Signature::new(name.clone(), Language::Python);

        if node.kind() == "class_definition" {
            // Extract base classes from inheritance
            if let Some(bases_node) = node.child_by_field_name("superclasses") {
                // Python doesn't have formal generics but we can extract base classes
                let bases = self.extract_base_classes(&bases_node, source);
                for base in bases {
                    // Store as constraints for now
                    signature = signature.with_constraint(base);
                }
            }

            // Build the class signature string
            let class_signature = self.build_class_signature(&name, node, source);
            signature = signature.with_raw_signature(class_signature);
        }

        Some(signature)
    }

    fn parse_type_info(&self, node: &Node, source: &str) -> Option<TypeInfo> {
        match node.kind() {
            "identifier" => {
                let type_name = self.get_node_text(node, source);
                Some(TypeInfo::new(type_name))
            }
            "attribute" => {
                // Handle qualified types like typing.List
                let type_name = self.get_node_text(node, source);
                Some(TypeInfo::new(type_name))
            }
            "subscript" => {
                // Handle generic types like List[str], Dict[str, int]
                let mut base_name = String::new();
                let mut generic_args = Vec::new();

                if let Some(value_node) = node.child_by_field_name("value") {
                    base_name = self.get_node_text(&value_node, source);
                }

                if let Some(slice_node) = node.child_by_field_name("slice") {
                    // Handle both single and multiple subscripts
                    generic_args = self.extract_subscript_args(&slice_node, source);
                }

                if !base_name.is_empty() {
                    Some(TypeInfo::generic(base_name, generic_args))
                } else {
                    None
                }
            }
            "list" => {
                // Handle List type annotations
                let mut list_types = Vec::new();
                for child in node.children(&mut node.walk()) {
                    if let Some(element_type) = self.parse_type_info(&child, source) {
                        list_types.push(element_type);
                    }
                }
                if list_types.len() == 1 {
                    Some(TypeInfo::array(list_types.into_iter().next().unwrap(), 1))
                } else {
                    Some(TypeInfo::new("list".to_string()))
                }
            }
            "binary_operator" => {
                // Handle Union types like str | int
                if self.get_node_text(node, source).contains('|') {
                    let union_types = self.extract_union_types(node, source);
                    if union_types.len() == 1 {
                        union_types.into_iter().next()
                    } else {
                        let union_str = union_types
                            .iter()
                            .map(|t| t.name.clone())
                            .collect::<Vec<_>>()
                            .join(" | ");
                        Some(TypeInfo::new(union_str))
                    }
                } else {
                    None
                }
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
            "identifier" => {
                let name = self.get_node_text(node, source);
                // Check if this is a special parameter like self, cls
                let param_type = if name == "self" {
                    TypeInfo::new("Self".to_string())
                } else if name == "cls" {
                    TypeInfo::new("Type[Self]".to_string())
                } else {
                    TypeInfo::new("Any".to_string())
                };
                Some(Parameter::new(name).with_type(param_type))
            }
            "typed_parameter" => {
                let mut param_name = String::new();
                let mut param_type = None;

                for child in node.children(&mut node.walk()) {
                    match child.kind() {
                        "identifier" => {
                            param_name = self.get_node_text(&child, source);
                        }
                        _ => {
                            // Try to parse as type annotation
                            if let Some(type_info) = self.parse_type_info(&child, source) {
                                param_type = Some(type_info);
                            }
                        }
                    }
                }

                if !param_name.is_empty() {
                    let mut parameter = Parameter::new(param_name);
                    if let Some(type_info) = param_type {
                        parameter = parameter.with_type(type_info);
                    } else {
                        parameter = parameter.with_type(TypeInfo::new("Any".to_string()));
                    }
                    Some(parameter)
                } else {
                    None
                }
            }
            "default_parameter" => {
                let mut param_name = String::new();
                let mut param_type = None;
                let mut default_value = None;

                for child in node.children(&mut node.walk()) {
                    match child.kind() {
                        "identifier" => {
                            param_name = self.get_node_text(&child, source);
                        }
                        "typed_parameter" => {
                            // Handle typed default parameters
                            if let Some(typed_param) = self.parse_parameter(&child, source) {
                                param_name = typed_param.name;
                                param_type = typed_param.type_info;
                            }
                        }
                        _ => {
                            // This might be the default value
                            if !param_name.is_empty() && default_value.is_none() {
                                default_value = Some(self.get_node_text(&child, source));
                            }
                        }
                    }
                }

                if !param_name.is_empty() {
                    let mut parameter = Parameter::new(param_name);
                    if let Some(type_info) = param_type {
                        parameter = parameter.with_type(type_info);
                    } else {
                        parameter = parameter.with_type(TypeInfo::new("Any".to_string()));
                    }
                    if let Some(default) = default_value {
                        parameter = parameter.with_default(default);
                    }
                    Some(parameter)
                } else {
                    None
                }
            }
            "list_splat_pattern" => {
                // Handle *args parameters
                for child in node.children(&mut node.walk()) {
                    if let Some(param) = self.parse_parameter(&child, source) {
                        let name = format!("*{}", param.name);
                        let param_type = TypeInfo::new("tuple".to_string());
                        return Some(Parameter::new(name).with_type(param_type).variadic());
                    }
                }
                None
            }
            "dictionary_splat_pattern" => {
                // Handle **kwargs parameters
                for child in node.children(&mut node.walk()) {
                    if let Some(param) = self.parse_parameter(&child, source) {
                        let name = format!("**{}", param.name);
                        let param_type = TypeInfo::new("dict".to_string());
                        return Some(Parameter::new(name).with_type(param_type).variadic());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn parse_generic_parameters(&self, _node: &Node, _source: &str) -> Vec<GenericParameter> {
        // Python doesn't have generics in the same way as other languages
        // But we could potentially extract TypeVar definitions
        Vec::new()
    }

    fn parse_modifiers(&self, node: &Node, source: &str) -> Vec<Modifier> {
        let mut modifiers = Vec::new();

        // Check for decorators that indicate modifiers
        if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                for child in parent.children(&mut parent.walk()) {
                    if child.kind() == "decorator" {
                        let decorator_text = self.get_node_text(&child, source);
                        match decorator_text.as_str() {
                            "@staticmethod" => modifiers.push(Modifier::Static),
                            "@classmethod" => modifiers.push(Modifier::ClassMethod),
                            "@property" => modifiers.push(Modifier::Property),
                            "@abstractmethod" => modifiers.push(Modifier::Abstract),
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check for async functions
        if self.is_async_function(node, source) {
            modifiers.push(Modifier::Async);
        }

        modifiers
    }
}

impl PythonExtractor {
    /// Check if a symbol (method) is within the source range of another symbol (class)
    fn is_symbol_within_range(inner: &OutlineNode, outer: &OutlineNode) -> bool {
        // A method belongs to a class if its byte range is completely within the class's byte range
        inner.source_range.0 >= outer.source_range.0
            && inner.source_range.1 <= outer.source_range.1
            && inner.start_line >= outer.start_line
            && inner.end_line <= outer.end_line
    }

    /// Check if a function is async
    fn is_async_function(&self, node: &Node, source: &str) -> bool {
        // Check if this is inside an async function
        let node_text = self.get_node_text(node, source);
        node_text.starts_with("async ")
    }

    /// Extract parameters from a parameters node
    fn extract_parameters_from_node(&self, node: &Node, source: &str) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        for child in node.children(&mut node.walk()) {
            if let Some(param) = self.parse_parameter(&child, source) {
                parameters.push(param);
            }
        }

        parameters
    }

    /// Parse return type from Python function
    fn parse_python_return_type(&self, node: &Node, source: &str) -> Option<TypeInfo> {
        // Look for return type annotation (->)
        for child in node.children(&mut node.walk()) {
            if child.kind() == "type" {
                return self.parse_type_info(&child, source);
            }
        }
        None
    }

    /// Extract base classes from a superclasses node
    fn extract_base_classes(&self, node: &Node, source: &str) -> Vec<String> {
        let mut bases = Vec::new();

        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" || child.kind() == "attribute" {
                bases.push(self.get_node_text(&child, source));
            }
        }

        bases
    }

    /// Extract subscript arguments for generic types
    fn extract_subscript_args(&self, node: &Node, source: &str) -> Vec<TypeInfo> {
        let mut args = Vec::new();

        for child in node.children(&mut node.walk()) {
            if let Some(type_info) = self.parse_type_info(&child, source) {
                args.push(type_info);
            }
        }

        args
    }

    /// Extract union types from binary operators
    fn extract_union_types(&self, node: &Node, source: &str) -> Vec<TypeInfo> {
        let mut types = Vec::new();

        for child in node.children(&mut node.walk()) {
            if child.kind() != "|" {
                if let Some(type_info) = self.parse_type_info(&child, source) {
                    types.push(type_info);
                }
            }
        }

        types
    }
}

impl SymbolExtractor for PythonExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Result<Vec<OutlineNode>> {
        let mut symbols = Vec::new();
        let mut seen_nodes = std::collections::HashSet::new();
        let root_node = tree.root_node();

        // Process each query
        for (node_type, query) in &self.queries {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(query, root_node, source.as_bytes());

            while let Some(query_match) = matches.next() {
                // Get the main captured node (should be the only capture)
                if let Some(capture) = query_match.captures.first() {
                    let node = &capture.node;

                    // For decorated definitions, we need to extract from the inner definition
                    let target_node = if node.kind() == "decorated_definition" {
                        // Find the actual function or class definition inside the decorated definition
                        node.child_by_field_name("definition").unwrap_or(*node)
                    } else {
                        *node
                    };

                    // Use the target_node (the actual function/class) for deduplication
                    let node_key = (
                        target_node.start_byte(),
                        target_node.end_byte(),
                        node_type.clone(),
                    );
                    if seen_nodes.contains(&node_key) {
                        continue; // Skip duplicate
                    }
                    seen_nodes.insert(node_key);

                    if let Some(name) = self.extract_name_from_node(&target_node, source) {
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
                                Some(self.build_function_signature(&name, &target_node, source))
                            }
                            OutlineNodeType::Class => {
                                Some(self.build_class_signature(&name, &target_node, source))
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
                        if let Some(docs) = self.extract_docstring(&target_node, source) {
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
        let mut hierarchical_symbols = Vec::new();
        let mut used_indices = std::collections::HashSet::new();

        // Sort symbols by start line for processing in order
        let mut sorted_symbols: Vec<(usize, &OutlineNode)> = symbols.iter().enumerate().collect();
        sorted_symbols.sort_by_key(|&(_, symbol)| symbol.start_line);

        // First pass: Process classes and nested classes
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Class {
                let mut class_symbol = symbol.clone();

                // Find all symbols that belong to this class (methods, properties, variables, nested classes)
                for &(j, potential_child) in &sorted_symbols {
                    if i != j && Self::is_symbol_within_range(potential_child, symbol) {
                        // Check if this child isn't already a child of a more specific parent
                        let mut should_add = true;

                        // For nested classes, ensure we don't double-assign to both parent and grandparent
                        if potential_child.node_type == OutlineNodeType::Class {
                            for &(k, other_class) in &sorted_symbols {
                                if k != i && other_class.node_type == OutlineNodeType::Class {
                                    // If there's another class that contains this child and is within our class
                                    if Self::is_symbol_within_range(potential_child, other_class)
                                        && Self::is_symbol_within_range(other_class, symbol)
                                    {
                                        should_add = false;
                                        break;
                                    }
                                }
                            }
                        }

                        // For functions, check if they belong to a nested class instead
                        if potential_child.node_type == OutlineNodeType::Function {
                            for &(k, other_class) in &sorted_symbols {
                                if k != i && other_class.node_type == OutlineNodeType::Class {
                                    // If there's a nested class that contains this function
                                    if Self::is_symbol_within_range(potential_child, other_class)
                                        && Self::is_symbol_within_range(other_class, symbol)
                                        && other_class.start_line > symbol.start_line
                                    {
                                        should_add = false;
                                        break;
                                    }
                                }
                            }
                        }

                        if should_add {
                            class_symbol.add_child(potential_child.clone());
                            used_indices.insert(j);
                        }
                    }
                }

                hierarchical_symbols.push(class_symbol);
                used_indices.insert(i);
            }
        }

        // Second pass: Process functions and their nested functions
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Function && !used_indices.contains(&i) {
                let mut function_symbol = symbol.clone();

                // Find nested functions within this function
                for &(j, potential_nested) in &sorted_symbols {
                    if i != j
                        && potential_nested.node_type == OutlineNodeType::Function
                        && !used_indices.contains(&j)
                        && Self::is_symbol_within_range(potential_nested, symbol)
                    {
                        function_symbol.add_child(potential_nested.clone());
                        used_indices.insert(j);
                    }
                }

                hierarchical_symbols.push(function_symbol);
                used_indices.insert(i);
            }
        }

        // Third pass: Add remaining symbols that weren't used as children
        for (i, symbol) in symbols.iter().enumerate() {
            if !used_indices.contains(&i) {
                hierarchical_symbols.push(symbol.clone());
            }
        }

        // Sort to maintain original order for top-level symbols
        hierarchical_symbols.sort_by_key(|s| s.start_line);
        hierarchical_symbols
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

    #[test]
    fn test_hierarchical_relationships() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
class User:
    """User class with methods."""
    
    def __init__(self, name: str):
        """Initialize user."""
        self.name = name
    
    def get_name(self) -> str:
        """Get the user name."""
        return self.name
    
    def set_name(self, name: str):
        """Set the user name."""
        self.name = name
    
    @property
    def display_name(self) -> str:
        """Get formatted display name."""
        return f"User: {self.name}"

def standalone_function():
    """Standalone function outside of class."""
    return "standalone"

class Database:
    """Database class."""
    
    def connect(self):
        """Connect to database."""
        pass
    
    def disconnect(self):
        """Disconnect from database."""
        pass
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        // Build hierarchy
        let hierarchical_symbols = extractor.build_hierarchy(symbols);

        // Print results for debugging
        println!("Hierarchical symbols:");
        for symbol in &hierarchical_symbols {
            println!(
                "  {:?} '{}' at line {} (children: {})",
                symbol.node_type,
                symbol.name,
                symbol.start_line,
                symbol.children.len()
            );
            for child in &symbol.children {
                println!(
                    "    └── {:?} '{}' at line {}",
                    child.node_type, child.name, child.start_line
                );
            }
        }

        // Verify classes are present with children
        let user_class = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "User")
            .expect("Should find User class");

        let database_class = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "Database")
            .expect("Should find Database class");

        // User class should have methods as children
        assert!(
            user_class.children.len() >= 4,
            "User class should have at least 4 methods (__init__, get_name, set_name, display_name), found: {}",
            user_class.children.len()
        );

        // Database class should have methods as children
        assert!(
            database_class.children.len() >= 2,
            "Database class should have at least 2 methods (connect, disconnect), found: {}",
            database_class.children.len()
        );

        // Verify specific methods are children of User class
        let user_child_names: Vec<&String> = user_class
            .children
            .iter()
            .map(|child| &child.name)
            .collect();
        assert!(
            user_child_names.contains(&&"__init__".to_string()),
            "User class should contain __init__ method"
        );
        assert!(
            user_child_names.contains(&&"get_name".to_string()),
            "User class should contain get_name method"
        );
        assert!(
            user_child_names.contains(&&"set_name".to_string()),
            "User class should contain set_name method"
        );
        assert!(
            user_child_names.contains(&&"display_name".to_string()),
            "User class should contain display_name method"
        );

        // Verify Database class methods
        let database_child_names: Vec<&String> = database_class
            .children
            .iter()
            .map(|child| &child.name)
            .collect();
        assert!(
            database_child_names.contains(&&"connect".to_string()),
            "Database class should contain connect method"
        );
        assert!(
            database_child_names.contains(&&"disconnect".to_string()),
            "Database class should contain disconnect method"
        );

        // Verify standalone function is not a child of any class
        let standalone_function = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Function && s.name == "standalone_function")
            .expect("Should find standalone_function at top level");

        assert_eq!(
            standalone_function.children.len(),
            0,
            "Standalone function should not have children"
        );

        // Verify we don't have methods at top level anymore (they should be nested)
        let top_level_methods: Vec<&OutlineNode> = hierarchical_symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Function && s.name != "standalone_function")
            .collect();

        assert!(
            top_level_methods.is_empty(),
            "Should not have class methods at top level, found: {:?}",
            top_level_methods
                .iter()
                .map(|s| &s.name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_nested_classes() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
class OuterClass:
    """Outer class."""
    
    def outer_method(self):
        """Method in outer class."""
        pass
    
    class InnerClass:
        """Inner class."""
        
        def inner_method(self):
            """Method in inner class."""
            pass
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        let hierarchical_symbols = extractor.build_hierarchy(symbols);

        // Should find outer class
        let outer_class = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "OuterClass")
            .expect("Should find OuterClass");

        // Should find inner class
        let inner_class = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "InnerClass")
            .expect("Should find InnerClass");

        // Outer class should have outer_method as child
        let outer_child_names: Vec<&String> = outer_class
            .children
            .iter()
            .map(|child| &child.name)
            .collect();
        assert!(
            outer_child_names.contains(&&"outer_method".to_string()),
            "OuterClass should contain outer_method"
        );

        // Inner class should have inner_method as child
        let inner_child_names: Vec<&String> = inner_class
            .children
            .iter()
            .map(|child| &child.name)
            .collect();
        assert!(
            inner_child_names.contains(&&"inner_method".to_string()),
            "InnerClass should contain inner_method"
        );
    }

    #[test]
    fn test_comprehensive_hierarchical_structure() {
        let extractor = PythonExtractor::new().unwrap();
        let source = r#"
"""Module level docstring."""

from typing import Optional, List
import sys

# Module level constants
VERSION = "1.0.0"
MAX_CONNECTIONS = 100

class BaseClass:
    """Base class with various member types."""
    
    # Class variable
    class_var: int = 42
    
    def __init__(self, name: str) -> None:
        """Initialize base class."""
        self.name = name
        self.instance_var = "instance"
    
    @property
    def name_property(self) -> str:
        """Get name as property."""
        return self.name
    
    @staticmethod
    def static_utility() -> str:
        """Static utility method."""
        return "static"
    
    @classmethod
    def create_default(cls) -> 'BaseClass':
        """Class method to create default instance."""
        return cls("default")
    
    def regular_method(self, param: str) -> Optional[str]:
        """Regular instance method."""
        return param
    
    async def async_method(self) -> None:
        """Async method."""
        pass
    
    def _private_method(self) -> None:
        """Private method."""
        pass
    
    def __dunder_method__(self) -> str:
        """Dunder method."""
        return "dunder"
    
    class NestedClass:
        """Nested class within BaseClass."""
        
        nested_var = "nested"
        
        def __init__(self) -> None:
            """Initialize nested class."""
            pass
        
        def nested_method(self) -> str:
            """Method in nested class."""
            return "nested"
        
        @property
        def nested_property(self) -> str:
            """Property in nested class."""
            return self.nested_var

@dataclass
class DataModel:
    """Data model class."""
    name: str
    value: int = 10
    
    def model_method(self) -> str:
        """Method in data model."""
        return f"{self.name}: {self.value}"

def standalone_function(arg1: str, arg2: int = 5) -> str:
    """Standalone function."""
    return f"{arg1}_{arg2}"

async def async_standalone(data: List[str]) -> None:
    """Async standalone function."""
    pass

def function_with_nested():
    """Function with nested function."""
    def inner_function():
        """Nested function."""
        return "inner"
    return inner_function()
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        let hierarchical_symbols = extractor.build_hierarchy(symbols);

        println!("Comprehensive hierarchical symbols:");
        for symbol in &hierarchical_symbols {
            println!(
                "  {:?} '{}' at line {} (children: {})",
                symbol.node_type,
                symbol.name,
                symbol.start_line,
                symbol.children.len()
            );
            for child in &symbol.children {
                println!(
                    "    └── {:?} '{}' at line {}",
                    child.node_type, child.name, child.start_line
                );
            }
        }

        // Verify BaseClass has all its methods and properties
        let base_class = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "BaseClass")
            .expect("Should find BaseClass");

        // BaseClass should have multiple children (methods and properties)
        assert!(
            base_class.children.len() >= 8,
            "BaseClass should have at least 8 children (methods and properties), found: {}",
            base_class.children.len()
        );

        // Check specific methods are children of BaseClass
        let base_child_names: Vec<&String> = base_class
            .children
            .iter()
            .map(|child| &child.name)
            .collect();

        let expected_base_methods = [
            "__init__",
            "name_property",
            "static_utility",
            "create_default",
            "regular_method",
            "async_method",
            "_private_method",
            "__dunder_method__",
        ];

        for method_name in &expected_base_methods {
            assert!(
                base_child_names.contains(&&method_name.to_string()),
                "BaseClass should contain {} method",
                method_name
            );
        }

        // Verify NestedClass exists and has its methods
        let nested_class = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "NestedClass")
            .expect("Should find NestedClass");

        assert!(
            nested_class.children.len() >= 2,
            "NestedClass should have at least 2 children, found: {}",
            nested_class.children.len()
        );

        let nested_child_names: Vec<&String> = nested_class
            .children
            .iter()
            .map(|child| &child.name)
            .collect();

        assert!(
            nested_child_names.contains(&&"__init__".to_string()),
            "NestedClass should contain __init__ method"
        );
        assert!(
            nested_child_names.contains(&&"nested_method".to_string()),
            "NestedClass should contain nested_method"
        );
        assert!(
            nested_child_names.contains(&&"nested_property".to_string()),
            "NestedClass should contain nested_property"
        );

        // Verify DataModel class
        let data_model = hierarchical_symbols
            .iter()
            .find(|s| s.node_type == OutlineNodeType::Class && s.name == "DataModel")
            .expect("Should find DataModel");

        assert!(
            data_model.children.len() >= 1,
            "DataModel should have at least 1 child method, found: {}",
            data_model.children.len()
        );

        // Verify standalone functions are at top level
        let standalone_functions: Vec<&OutlineNode> = hierarchical_symbols
            .iter()
            .filter(|s| {
                s.node_type == OutlineNodeType::Function
                    && matches!(
                        s.name.as_str(),
                        "standalone_function" | "async_standalone" | "function_with_nested"
                    )
            })
            .collect();

        assert_eq!(
            standalone_functions.len(),
            3,
            "Should find 3 standalone functions, found: {}",
            standalone_functions.len()
        );

        // Verify simple standalone functions have no children
        let simple_standalone: Vec<&OutlineNode> = standalone_functions
            .iter()
            .filter(|s| matches!(s.name.as_str(), "standalone_function" | "async_standalone"))
            .copied()
            .collect();

        for func in &simple_standalone {
            assert_eq!(
                func.children.len(),
                0,
                "Simple standalone function '{}' should not have children",
                func.name
            );
        }

        // Verify function_with_nested has nested function as child
        let function_with_nested = standalone_functions
            .iter()
            .find(|s| s.name == "function_with_nested")
            .expect("Should find function_with_nested");

        assert_eq!(
            function_with_nested.children.len(),
            1,
            "function_with_nested should have 1 child (inner_function)"
        );

        let nested_child_names: Vec<&String> = function_with_nested
            .children
            .iter()
            .map(|child| &child.name)
            .collect();
        assert!(
            nested_child_names.contains(&&"inner_function".to_string()),
            "function_with_nested should contain inner_function as child"
        );

        // Verify no methods appear at top level (except standalone functions)
        let top_level_functions: Vec<&OutlineNode> = hierarchical_symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Function)
            .collect();

        // All top-level functions should be standalone (not class methods or nested functions)
        for func in &top_level_functions {
            assert!(
                matches!(func.name.as_str(), "standalone_function" | "async_standalone" | "function_with_nested"),
                "Found unexpected top-level function: {} (nested functions and class methods should be children)",
                func.name
            );
        }

        // Verify imports and variables exist at module level
        let imports: Vec<&OutlineNode> = hierarchical_symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Import)
            .collect();
        assert!(!imports.is_empty(), "Should find import statements");

        let variables: Vec<&OutlineNode> = hierarchical_symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Variable)
            .collect();
        assert!(!variables.is_empty(), "Should find module-level variables");
    }
}
