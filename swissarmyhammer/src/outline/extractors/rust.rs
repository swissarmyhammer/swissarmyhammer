//! Rust language symbol extractor for outline generation
//!
//! This module implements Tree-sitter based symbol extraction for Rust code,
//! supporting structs, enums, traits, impls, functions, methods, constants,
//! and their associated documentation, visibility, and signature information.

use crate::outline::parser::SymbolExtractor;
use crate::outline::signature::{
    GenericParameter, Modifier, Parameter, Signature, SignatureExtractor, TypeInfo,
};
use crate::outline::types::{OutlineNode, OutlineNodeType, Visibility};
use crate::outline::{OutlineError, Result};
use swissarmyhammer_search::Language;
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
            (OutlineNodeType::Function, r#"(function_item) @function"#),
            (OutlineNodeType::Struct, r#"(struct_item) @struct"#),
            (OutlineNodeType::Enum, r#"(enum_item) @enum"#),
            (OutlineNodeType::Trait, r#"(trait_item) @trait"#),
            (OutlineNodeType::Impl, r#"(impl_item) @impl"#),
            (OutlineNodeType::Constant, r#"(const_item) @const"#),
            (OutlineNodeType::Variable, r#"(static_item) @static"#),
            (OutlineNodeType::TypeAlias, r#"(type_item) @type_alias"#),
            (OutlineNodeType::Module, r#"(mod_item) @module"#),
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
                        impl_name = format!("impl {type_name}");
                        found_trait = true;
                    } else if found_trait && !found_type {
                        // Second type_identifier is the type being implemented for
                        impl_name = format!("{impl_name} for {type_name}");
                        found_type = true;
                    }
                }
                "generic_type" => {
                    // Handle generic types in impl blocks
                    let generic_text = self.get_node_text(&child, source);
                    if !found_type {
                        if found_trait {
                            impl_name = format!("{impl_name} for {generic_text}");
                        } else {
                            impl_name = format!("impl {generic_text}");
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
            signature.push_str(ret.strip_prefix(": ").unwrap_or(&ret));
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
            if child.kind() == "type_identifier" {
                let type_name = self.get_node_text(&child, source);
                signature.push(' ');
                signature.push_str(&type_name);
            }
        }

        signature
    }

    /// Check if an inner symbol is within the range of an outer symbol
    fn is_symbol_within_range(inner: &OutlineNode, outer: &OutlineNode) -> bool {
        // A symbol belongs to a container if its byte range is completely within the container's byte range
        inner.source_range.0 >= outer.source_range.0
            && inner.source_range.1 <= outer.source_range.1
            && inner.start_line >= outer.start_line
            && inner.end_line <= outer.end_line
    }
}

impl SignatureExtractor for RustExtractor {
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        if node.kind() != "function_item" {
            return None;
        }

        let name = self.extract_name_from_node(node, source)?;
        let mut signature = Signature::new(name.clone(), Language::Rust);

        // Extract visibility modifiers
        let mut modifiers = Vec::new();
        if let Some(visibility) = self.parse_visibility(node, source) {
            match visibility {
                Visibility::Public => modifiers.push(Modifier::Public),
                Visibility::Private => {}   // Private is implicit in Rust
                Visibility::Protected => {} // Not applicable to Rust
                Visibility::Package => modifiers.push(Modifier::Public), // pub(crate) etc.
                Visibility::Module => modifiers.push(Modifier::Public),
                Visibility::Custom(_) => modifiers.push(Modifier::Public),
            }
        }

        // Check for async, unsafe, const, extern modifiers
        let node_text = self.get_node_text(node, source);
        if node_text.contains("async") {
            modifiers.push(Modifier::Async);
            signature = signature.async_function();
        }
        if node_text.contains("unsafe") {
            modifiers.push(Modifier::Unsafe);
        }
        if node_text.contains("const") {
            modifiers.push(Modifier::Const);
        }
        if node_text.contains("extern") {
            modifiers.push(Modifier::Extern);
        }

        signature = signature.with_modifiers(modifiers);

        // Extract generic parameters
        let generics = self.parse_generic_parameters(node, source);
        for generic in generics {
            signature = signature.with_generic(generic);
        }

        // Extract parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            for param_node in params_node.children(&mut params_node.walk()) {
                if let Some(parameter) = self.parse_parameter(&param_node, source) {
                    signature = signature.with_parameter(parameter);
                }
            }
        }

        // Extract return type
        if let Some(return_type) = self.parse_type_info_from_return_type(node, source) {
            signature = signature.with_return_type(return_type);
        }

        // Set raw signature
        signature =
            signature.with_raw_signature(self.build_function_signature(&name, node, source));

        Some(signature)
    }

    fn extract_method_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        // Methods are just functions inside impl blocks
        self.extract_function_signature(node, source)
    }

    fn extract_constructor_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        // Rust doesn't have explicit constructors, but we can treat `new` functions as constructors
        if let Some(mut signature) = self.extract_function_signature(node, source) {
            if signature.name == "new" {
                signature = signature.constructor();
            }
            Some(signature)
        } else {
            None
        }
    }

    fn extract_type_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        match node.kind() {
            "struct_item" => {
                let name = self.extract_name_from_node(node, source)?;
                let mut signature = Signature::new(name.clone(), Language::Rust);

                // Add visibility modifiers
                let mut modifiers = Vec::new();
                if let Some(Visibility::Public) = self.parse_visibility(node, source) {
                    modifiers.push(Modifier::Public);
                }
                signature = signature.with_modifiers(modifiers);

                // Extract generic parameters
                let generics = self.parse_generic_parameters(node, source);
                for generic in generics {
                    signature = signature.with_generic(generic);
                }

                signature =
                    signature.with_raw_signature(self.build_struct_signature(&name, node, source));
                Some(signature)
            }
            "enum_item" | "trait_item" | "type_item" => {
                let name = self.extract_name_from_node(node, source)?;
                let mut signature = Signature::new(name.clone(), Language::Rust);

                // Add visibility modifiers
                let mut modifiers = Vec::new();
                if let Some(visibility) = self.parse_visibility(node, source) {
                    if visibility == Visibility::Public {
                        modifiers.push(Modifier::Public);
                    }
                }
                signature = signature.with_modifiers(modifiers);

                // Extract generic parameters
                let generics = self.parse_generic_parameters(node, source);
                for generic in generics {
                    signature = signature.with_generic(generic);
                }

                let raw_sig = match node.kind() {
                    "enum_item" => self.build_enum_signature(&name, node, source),
                    "trait_item" => self.build_trait_signature(&name, node, source),
                    "type_item" => format!("type {name}"),
                    _ => name.clone(),
                };
                signature = signature.with_raw_signature(raw_sig);
                Some(signature)
            }
            _ => None,
        }
    }

    fn parse_type_info(&self, node: &Node, source: &str) -> Option<TypeInfo> {
        match node.kind() {
            "type_identifier" => {
                let name = self.get_node_text(node, source);
                Some(TypeInfo::new(name))
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
            "reference_type" => {
                // Handle &T, &mut T
                for child in node.children(&mut node.walk()) {
                    if child.kind() != "&" && child.kind() != "mut" {
                        if let Some(inner_type) = self.parse_type_info(&child, source) {
                            let type_name =
                                if node.children(&mut node.walk()).any(|c| c.kind() == "mut") {
                                    format!("&mut {}", inner_type.name)
                                } else {
                                    format!("&{}", inner_type.name)
                                };
                            return Some(TypeInfo::new(type_name));
                        }
                    }
                }
                None
            }
            "array_type" => {
                // Handle [T; N] or [T]
                for child in node.children(&mut node.walk()) {
                    if let Some(element_type) = self.parse_type_info(&child, source) {
                        return Some(TypeInfo::array(element_type, 1));
                    }
                }
                None
            }
            "tuple_type" => {
                // Handle (T, U, V)
                let mut tuple_types = Vec::new();
                for child in node.children(&mut node.walk()) {
                    if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                        if let Some(element_type) = self.parse_type_info(&child, source) {
                            tuple_types.push(element_type);
                        }
                    }
                }
                if !tuple_types.is_empty() {
                    let tuple_name = format!(
                        "({})",
                        tuple_types
                            .iter()
                            .map(|t| t.name.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    Some(TypeInfo::new(tuple_name))
                } else {
                    None
                }
            }
            "function_type" => {
                // Handle function pointer types: fn(args) -> ret
                let mut param_types = Vec::new();
                let mut return_type = None;

                for child in node.children(&mut node.walk()) {
                    match child.kind() {
                        "parameters" => {
                            for param_child in child.children(&mut child.walk()) {
                                if let Some(param_type) = self.parse_type_info(&param_child, source)
                                {
                                    param_types.push(param_type);
                                }
                            }
                        }
                        "type_identifier" | "generic_type" => {
                            // This might be the return type
                            if let Some(ret_type) = self.parse_type_info(&child, source) {
                                return_type = Some(ret_type);
                            }
                        }
                        _ => {}
                    }
                }

                Some(TypeInfo::function(param_types, return_type))
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
            "parameter" => {
                // Extract parameter name and type
                let mut param_name = String::new();
                let mut param_type = None;
                let mut modifiers = Vec::new();

                for child in node.children(&mut node.walk()) {
                    match child.kind() {
                        "identifier" => {
                            param_name = self.get_node_text(&child, source);
                        }
                        "mut" => {
                            modifiers.push(Modifier::Mut);
                        }
                        "ref" => {
                            modifiers.push(Modifier::Ref);
                        }
                        _ => {
                            // Try to parse as type information
                            if let Some(type_info) = self.parse_type_info(&child, source) {
                                param_type = Some(type_info);
                            }
                        }
                    }
                }

                if !param_name.is_empty() {
                    let is_mutable = modifiers.contains(&Modifier::Mut);
                    let mut parameter = Parameter::new(param_name).with_modifiers(modifiers);
                    if let Some(type_info) = param_type {
                        parameter = parameter.with_type(type_info);
                    }
                    if is_mutable {
                        parameter = parameter.mutable();
                    }
                    Some(parameter)
                } else {
                    None
                }
            }
            "self_parameter" => {
                // Handle self, &self, &mut self
                let self_text = self.get_node_text(node, source);
                let mut parameter = Parameter::new("self".to_string());

                if self_text.contains("mut") {
                    parameter = parameter.mutable().with_modifiers(vec![Modifier::Mut]);
                }

                let type_name = if self_text.starts_with("&mut") {
                    "&mut Self"
                } else if self_text.starts_with('&') {
                    "&Self"
                } else {
                    "Self"
                };

                parameter = parameter.with_type(TypeInfo::new(type_name.to_string()));
                Some(parameter)
            }
            _ => None,
        }
    }

    fn parse_generic_parameters(&self, node: &Node, source: &str) -> Vec<GenericParameter> {
        let mut generics = Vec::new();

        // Look for type_parameters node
        if let Some(type_params_node) = node.child_by_field_name("type_parameters") {
            for child in type_params_node.children(&mut type_params_node.walk()) {
                if child.kind() == "type_identifier" {
                    let name = self.get_node_text(&child, source);
                    generics.push(GenericParameter::new(name));
                } else if child.kind() == "constrained_type_parameter" {
                    // Handle T: Bound syntax
                    let mut param_name = String::new();
                    let mut bounds = Vec::new();

                    for bound_child in child.children(&mut child.walk()) {
                        match bound_child.kind() {
                            "type_identifier" => {
                                if param_name.is_empty() {
                                    param_name = self.get_node_text(&bound_child, source);
                                } else {
                                    bounds.push(self.get_node_text(&bound_child, source));
                                }
                            }
                            "trait_bounds" => {
                                for trait_child in bound_child.children(&mut bound_child.walk()) {
                                    if trait_child.kind() == "type_identifier" {
                                        bounds.push(self.get_node_text(&trait_child, source));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if !param_name.is_empty() {
                        generics.push(GenericParameter::new(param_name).with_bounds(bounds));
                    }
                }
            }
        }

        generics
    }

    fn parse_modifiers(&self, node: &Node, source: &str) -> Vec<Modifier> {
        let mut modifiers = Vec::new();
        let node_text = self.get_node_text(node, source);

        // Check for various Rust modifiers
        if node_text.contains("pub") {
            modifiers.push(Modifier::Public);
        }
        if node_text.contains("async") {
            modifiers.push(Modifier::Async);
        }
        if node_text.contains("unsafe") {
            modifiers.push(Modifier::Unsafe);
        }
        if node_text.contains("const") {
            modifiers.push(Modifier::Const);
        }
        if node_text.contains("extern") {
            modifiers.push(Modifier::Extern);
        }
        if node_text.contains("static") {
            modifiers.push(Modifier::Static);
        }

        modifiers
    }
}

impl RustExtractor {
    /// Parse type info specifically from return type field
    fn parse_type_info_from_return_type(&self, node: &Node, source: &str) -> Option<TypeInfo> {
        if let Some(return_type_node) = node.child_by_field_name("return_type") {
            // The return_type field might directly contain the type or have children
            // First try to parse the return_type_node itself
            if let Some(type_info) = self.parse_type_info(&return_type_node, source) {
                return Some(type_info);
            }

            // If that doesn't work, look through its children
            for child in return_type_node.children(&mut return_type_node.walk()) {
                if child.kind() != "->" && child.kind() != " " {
                    if let Some(type_info) = self.parse_type_info(&child, source) {
                        return Some(type_info);
                    }
                }
            }
        }
        None
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
                            (node.start_byte(), node.end_byte()),
                        );

                        // Add signature using enhanced signature extraction
                        let signature = match node_type {
                            OutlineNodeType::Function => {
                                // Use new comprehensive signature extraction
                                if let Some(detailed_sig) =
                                    self.extract_function_signature(node, source)
                                {
                                    Some(detailed_sig.format_for_language(Language::Rust))
                                } else {
                                    Some(self.build_function_signature(&name, node, source))
                                }
                            }
                            OutlineNodeType::Struct
                            | OutlineNodeType::Enum
                            | OutlineNodeType::Trait => {
                                // Use new type signature extraction
                                if let Some(detailed_sig) =
                                    self.extract_type_signature(node, source)
                                {
                                    Some(detailed_sig.format_for_language(Language::Rust))
                                } else {
                                    match node_type {
                                        OutlineNodeType::Struct => {
                                            Some(self.build_struct_signature(&name, node, source))
                                        }
                                        OutlineNodeType::Enum => {
                                            Some(self.build_enum_signature(&name, node, source))
                                        }
                                        OutlineNodeType::Trait => {
                                            Some(self.build_trait_signature(&name, node, source))
                                        }
                                        _ => None,
                                    }
                                }
                            }
                            OutlineNodeType::Impl => Some(self.build_impl_signature(node, source)),
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
                // Use new comprehensive signature extraction for functions
                if let Some(detailed_sig) = self.extract_function_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Rust))
                } else if let Some(name_node) = node.child_by_field_name("name") {
                    let name = self.get_node_text(&name_node, source);
                    Some(self.build_function_signature(&name, node, source))
                } else {
                    None
                }
            }
            "struct_item" | "enum_item" | "trait_item" | "type_item" => {
                // Use new comprehensive type signature extraction
                if let Some(detailed_sig) = self.extract_type_signature(node, source) {
                    Some(detailed_sig.format_for_language(Language::Rust))
                } else {
                    // Fallback to old method
                    if let Some(name_node) = node.child_by_field_name("name") {
                        let name = self.get_node_text(&name_node, source);
                        match node.kind() {
                            "struct_item" => Some(self.build_struct_signature(&name, node, source)),
                            "enum_item" => Some(self.build_enum_signature(&name, node, source)),
                            "trait_item" => Some(self.build_trait_signature(&name, node, source)),
                            "type_item" => Some(format!("type {name}")),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
            }
            "impl_item" => Some(self.build_impl_signature(node, source)),
            _ => None,
        }
    }

    fn extract_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        self.parse_visibility(node, source)
    }

    fn build_hierarchy(&self, symbols: Vec<OutlineNode>) -> Vec<OutlineNode> {
        let mut hierarchical_symbols = Vec::new();
        let mut used_indices = std::collections::HashSet::new();

        // Sort symbols by start line for processing in order
        let mut sorted_symbols: Vec<(usize, &OutlineNode)> = symbols.iter().enumerate().collect();
        sorted_symbols.sort_by_key(|&(_, symbol)| symbol.start_line);

        // First pass: Process traits and their associated methods
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Trait {
                let mut trait_symbol = symbol.clone();

                // Find methods that belong to this trait
                for &(j, potential_child) in &sorted_symbols {
                    if i != j
                        && !used_indices.contains(&j)
                        && Self::is_symbol_within_range(potential_child, symbol)
                    {
                        // Only add functions/methods that belong to this trait
                        if potential_child.node_type == OutlineNodeType::Function {
                            trait_symbol.add_child(potential_child.clone());
                            used_indices.insert(j);
                        }
                    }
                }

                hierarchical_symbols.push(trait_symbol);
                used_indices.insert(i);
            }
        }

        // Second pass: Process impl blocks and associate their methods
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Impl && !used_indices.contains(&i) {
                let mut impl_symbol = symbol.clone();

                // Find methods and associated items that belong to this impl block
                for &(j, potential_child) in &sorted_symbols {
                    if i != j
                        && !used_indices.contains(&j)
                        && Self::is_symbol_within_range(potential_child, symbol)
                    {
                        // Add functions, constants, and type aliases that belong to this impl
                        match potential_child.node_type {
                            OutlineNodeType::Function
                            | OutlineNodeType::Constant
                            | OutlineNodeType::TypeAlias => {
                                impl_symbol.add_child(potential_child.clone());
                                used_indices.insert(j);
                            }
                            _ => {} // Don't add other types as children to impls
                        }
                    }
                }

                hierarchical_symbols.push(impl_symbol);
                used_indices.insert(i);
            }
        }

        // Third pass: Process modules and their remaining contents
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Module && !used_indices.contains(&i) {
                let mut module_symbol = symbol.clone();

                // Find all symbols that belong to this module
                for &(j, potential_child) in &sorted_symbols {
                    if i != j
                        && !used_indices.contains(&j)
                        && Self::is_symbol_within_range(potential_child, symbol)
                    {
                        // Check if this child isn't already a child of a more specific parent
                        let mut should_add = true;

                        // For nested modules, ensure we don't double-assign to both parent and grandparent
                        if potential_child.node_type == OutlineNodeType::Module {
                            for &(k, other_module) in &sorted_symbols {
                                if k != i && other_module.node_type == OutlineNodeType::Module {
                                    // If there's another module that contains this child and is within our module
                                    if Self::is_symbol_within_range(potential_child, other_module)
                                        && Self::is_symbol_within_range(other_module, symbol)
                                    {
                                        should_add = false;
                                        break;
                                    }
                                }
                            }
                        }

                        if should_add {
                            module_symbol.add_child(potential_child.clone());
                            used_indices.insert(j);
                        }
                    }
                }

                hierarchical_symbols.push(module_symbol);
                used_indices.insert(i);
            }
        }

        // Fourth pass: Process enums and their variants/methods (if any)
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Enum && !used_indices.contains(&i) {
                let enum_symbol = symbol.clone();

                // Find associated items that belong to this enum (mainly from impl blocks)
                // Note: Enum variants are typically not captured as separate symbols by our queries
                // but enum methods from impl blocks would be handled in the impl pass above

                hierarchical_symbols.push(enum_symbol);
                used_indices.insert(i);
            }
        }

        // Fifth pass: Process structs
        for &(i, symbol) in &sorted_symbols {
            if symbol.node_type == OutlineNodeType::Struct && !used_indices.contains(&i) {
                let struct_symbol = symbol.clone();

                // Note: Struct methods are typically in impl blocks, which are handled separately
                // Struct fields are not typically captured as separate symbols by our queries

                hierarchical_symbols.push(struct_symbol);
                used_indices.insert(i);
            }
        }

        // Sixth pass: Add any remaining symbols that weren't processed
        for &(i, symbol) in &sorted_symbols {
            if !used_indices.contains(&i) {
                hierarchical_symbols.push(symbol.clone());
            }
        }

        hierarchical_symbols
    }

    fn get_queries(&self) -> Vec<(&'static str, OutlineNodeType)> {
        vec![
            // Functions
            ("(function_item) @function", OutlineNodeType::Function),
            // Structs
            ("(struct_item) @struct", OutlineNodeType::Struct),
            // Enums
            ("(enum_item) @enum", OutlineNodeType::Enum),
            // Traits
            ("(trait_item) @trait", OutlineNodeType::Trait),
            // Impl blocks
            ("(impl_item) @impl", OutlineNodeType::Impl),
            // Constants
            ("(const_item) @const", OutlineNodeType::Constant),
            // Static items
            ("(static_item) @static", OutlineNodeType::Variable),
            // Type aliases
            ("(type_item) @type_alias", OutlineNodeType::TypeAlias),
            // Modules
            ("(mod_item) @module", OutlineNodeType::Module),
        ]
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
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
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
            .contains("fn hello_world()"));
        assert!(func.signature.as_ref().unwrap().contains("-> String"));
        assert_eq!(
            func.documentation.as_ref().unwrap(),
            "This is a test function"
        );
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
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        assert_eq!(symbols.len(), 1);
        let struct_node = &symbols[0];
        assert_eq!(struct_node.name, "Person");
        assert_eq!(struct_node.node_type, OutlineNodeType::Struct);
        assert_eq!(struct_node.visibility, Some(Visibility::Public));
        assert!(struct_node
            .signature
            .as_ref()
            .unwrap()
            .contains("struct Person"));
        assert_eq!(
            struct_node.documentation.as_ref().unwrap(),
            "A simple struct"
        );
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
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
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
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        assert_eq!(symbols.len(), 1);
        let trait_node = &symbols[0];
        assert_eq!(trait_node.name, "Display");
        assert_eq!(trait_node.node_type, OutlineNodeType::Trait);
        assert_eq!(trait_node.visibility, Some(Visibility::Public));
        assert!(trait_node
            .signature
            .as_ref()
            .unwrap()
            .contains("trait Display"));
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
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
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
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();

        // Should extract multiple symbol types
        assert!(!symbols.is_empty());

        // Check that we got various types
        let types: std::collections::HashSet<&OutlineNodeType> =
            symbols.iter().map(|s| &s.node_type).collect();

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
            assert!(
                types.contains(&expected),
                "Missing symbol type: {expected:?}"
            );
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
        let public_symbols: Vec<&OutlineNode> = symbols
            .iter()
            .filter(|s| s.visibility == Some(Visibility::Public))
            .collect();
        assert!(!public_symbols.is_empty());

        // Check that some signatures contain generics
        let has_generics = symbols.iter().any(|s| {
            s.signature
                .as_ref()
                .is_some_and(|sig| sig.contains("<") && sig.contains(">"))
        });
        assert!(has_generics, "Should find symbols with generic parameters");

        // Check that some documentation was extracted
        let has_docs = symbols.iter().any(|s| s.documentation.is_some());
        assert!(has_docs, "Should find symbols with documentation");

        println!(
            "Successfully extracted {} symbols from complex Rust code",
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
        let extractor = RustExtractor::new().unwrap();
        let source = r#"
/// A simple module
pub mod my_module {
    /// A struct in the module
    pub struct Person {
        name: String,
        age: u32,
    }

    /// Implementation for Person
    impl Person {
        /// Create a new person
        pub fn new(name: String, age: u32) -> Self {
            Self { name, age }
        }

        /// Get the person's name
        pub fn name(&self) -> &str {
            &self.name
        }
    }

    /// A trait in the module
    pub trait Displayable {
        /// Display the object
        fn display(&self) -> String;
    }

    /// Trait implementation
    impl Displayable for Person {
        fn display(&self) -> String {
            format!("{} ({})", self.name, self.age)
        }
    }
}

/// A global function
pub fn main() {
    println!("Hello, World!");
}
        "#;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();

        let symbols = extractor.extract_symbols(&tree, source).unwrap();
        let hierarchical_symbols = extractor.build_hierarchy(symbols);

        // Find the module
        let module_opt = hierarchical_symbols
            .iter()
            .find(|s| s.name == "my_module" && s.node_type == OutlineNodeType::Module);
        assert!(module_opt.is_some(), "Should find my_module");

        let module = module_opt.unwrap();
        assert!(!module.children.is_empty(), "Module should have children");

        // Check that the module contains expected children (struct should be there)
        let child_names: Vec<&String> = module.children.iter().map(|c| &c.name).collect();
        assert!(
            child_names.contains(&&"Person".to_string()),
            "Module should contain Person struct"
        );

        // Find trait at top level (traits are processed first now)
        let trait_opt = hierarchical_symbols
            .iter()
            .find(|s| s.name == "Displayable" && s.node_type == OutlineNodeType::Trait);
        assert!(
            trait_opt.is_some(),
            "Should find Displayable trait at top level"
        );

        // Find impl blocks at top level (processed before modules)
        let impl_blocks: Vec<&OutlineNode> = hierarchical_symbols
            .iter()
            .filter(|s| s.node_type == OutlineNodeType::Impl)
            .collect();
        assert_eq!(impl_blocks.len(), 2, "Should have two impl blocks");

        // Check that impl blocks have their methods as children
        let person_impl = impl_blocks.iter().find(|impl_block| {
            impl_block.name.contains("Person") && !impl_block.name.contains("Displayable")
        });
        assert!(person_impl.is_some(), "Should find impl block for Person");

        if let Some(impl_block) = person_impl {
            let method_names: Vec<&String> = impl_block.children.iter().map(|c| &c.name).collect();
            assert!(
                method_names.contains(&&"new".to_string()),
                "Impl should contain new method"
            );
            assert!(
                method_names.contains(&&"name".to_string()),
                "Impl should contain name method"
            );
        }

        // Check that the trait impl has the display method
        let trait_impl = impl_blocks
            .iter()
            .find(|impl_block| impl_block.name.contains("Displayable"));
        assert!(
            trait_impl.is_some(),
            "Should find impl block for Displayable trait"
        );

        if let Some(impl_block) = trait_impl {
            let method_names: Vec<&String> = impl_block.children.iter().map(|c| &c.name).collect();
            assert!(
                method_names.contains(&&"display".to_string()),
                "Trait impl should contain display method"
            );
        }

        // Check that global function is not in the module
        let global_func = hierarchical_symbols
            .iter()
            .find(|s| s.name == "main" && s.node_type == OutlineNodeType::Function);
        assert!(global_func.is_some(), "Should find global main function");
        assert!(
            global_func.unwrap().children.is_empty(),
            "Global function should not have children"
        );

        println!("Hierarchical relationships test passed!");
        println!(
            "Module '{}' has {} children:",
            module.name,
            module.children.len()
        );
        for child in &module.children {
            println!(
                "  {:?} '{}' with {} children",
                child.node_type,
                child.name,
                child.children.len()
            );
            for grandchild in &child.children {
                println!("    {:?} '{}'", grandchild.node_type, grandchild.name);
            }
        }
    }
}
