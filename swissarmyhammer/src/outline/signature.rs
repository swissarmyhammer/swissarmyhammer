//! Comprehensive signature extraction system for all supported languages
//!
//! This module provides a unified signature extraction framework that handles
//! complex scenarios across all languages, generating accurate function/method
//! signatures with complete type information, parameter details, and return types.

use crate::search::types::Language;
use serde::{Deserialize, Serialize};
use std::fmt;
use tree_sitter::Node;

/// Visibility and access modifiers for symbols
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Modifier {
    /// Public visibility
    Public,
    /// Private visibility
    Private,
    /// Protected visibility
    Protected,
    /// Static modifier
    Static,
    /// Abstract modifier
    Abstract,
    /// Final modifier
    Final,
    /// Async modifier
    Async,
    /// Const modifier
    Const,
    /// Readonly modifier
    Readonly,
    /// Override modifier
    Override,
    /// Virtual modifier
    Virtual,
    /// Unsafe modifier (Rust)
    Unsafe,
    /// Extern modifier (Rust)
    Extern,
    /// Inline modifier
    Inline,
    /// Mut modifier (Rust)
    Mut,
    /// Ref modifier
    Ref,
}

impl Modifier {
    /// Format modifier for Rust syntax
    pub fn as_rust_str(&self) -> &'static str {
        match self {
            Modifier::Public => "pub",
            Modifier::Private => "", // Rust private is implicit
            Modifier::Static => "static",
            Modifier::Async => "async",
            Modifier::Const => "const",
            Modifier::Unsafe => "unsafe",
            Modifier::Extern => "extern",
            Modifier::Inline => "inline",
            Modifier::Mut => "mut",
            Modifier::Ref => "ref",
            _ => "",
        }
    }

    /// Format modifier for TypeScript syntax
    pub fn as_typescript_str(&self) -> &'static str {
        match self {
            Modifier::Public => "public",
            Modifier::Private => "private",
            Modifier::Protected => "protected",
            Modifier::Static => "static",
            Modifier::Abstract => "abstract",
            Modifier::Async => "async",
            Modifier::Readonly => "readonly",
            Modifier::Override => "override",
            _ => "",
        }
    }

    /// Format modifier for JavaScript syntax
    pub fn as_javascript_str(&self) -> &'static str {
        match self {
            Modifier::Static => "static",
            Modifier::Async => "async",
            _ => "",
        }
    }

    /// Format modifier for Python syntax
    pub fn as_python_str(&self) -> &'static str {
        match self {
            Modifier::Static => "@staticmethod",
            Modifier::Abstract => "@abstractmethod",
            Modifier::Async => "async",
            _ => "",
        }
    }

    /// Format modifier for Dart syntax
    pub fn as_dart_str(&self) -> &'static str {
        match self {
            Modifier::Static => "static",
            Modifier::Abstract => "abstract",
            Modifier::Async => "async",
            Modifier::Final => "final",
            Modifier::Const => "const",
            _ => "",
        }
    }
}

/// Type information with support for complex types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Base type name
    pub name: String,
    /// Generic type arguments
    pub generic_args: Vec<TypeInfo>,
    /// Whether the type is nullable
    pub is_nullable: bool,
    /// Whether the type is an array/list
    pub is_array: bool,
    /// Array dimensions (for multi-dimensional arrays)
    pub array_dimensions: usize,
    /// Type constraints or bounds
    pub constraints: Vec<String>,
    /// Whether this is a function type
    pub is_function_type: bool,
    /// Function type parameters (if this is a function type)
    pub function_params: Vec<TypeInfo>,
    /// Function return type (if this is a function type)
    pub function_return: Option<Box<TypeInfo>>,
}

impl TypeInfo {
    /// Create a new simple type
    pub fn new(name: String) -> Self {
        Self {
            name,
            generic_args: Vec::new(),
            is_nullable: false,
            is_array: false,
            array_dimensions: 0,
            constraints: Vec::new(),
            is_function_type: false,
            function_params: Vec::new(),
            function_return: None,
        }
    }

    /// Create a generic type with arguments
    pub fn generic(name: String, args: Vec<TypeInfo>) -> Self {
        Self {
            name,
            generic_args: args,
            is_nullable: false,
            is_array: false,
            array_dimensions: 0,
            constraints: Vec::new(),
            is_function_type: false,
            function_params: Vec::new(),
            function_return: None,
        }
    }

    /// Create an array type
    pub fn array(element_type: TypeInfo, dimensions: usize) -> Self {
        Self {
            name: element_type.name,
            generic_args: element_type.generic_args,
            is_nullable: element_type.is_nullable,
            is_array: true,
            array_dimensions: dimensions,
            constraints: element_type.constraints,
            is_function_type: element_type.is_function_type,
            function_params: element_type.function_params,
            function_return: element_type.function_return,
        }
    }

    /// Create a function type
    pub fn function(params: Vec<TypeInfo>, return_type: Option<TypeInfo>) -> Self {
        Self {
            name: "Function".to_string(),
            generic_args: Vec::new(),
            is_nullable: false,
            is_array: false,
            array_dimensions: 0,
            constraints: Vec::new(),
            is_function_type: true,
            function_params: params,
            function_return: return_type.map(Box::new),
        }
    }

    /// Make this type nullable
    pub fn nullable(mut self) -> Self {
        self.is_nullable = true;
        self
    }

    /// Add constraints to this type
    pub fn with_constraints(mut self, constraints: Vec<String>) -> Self {
        self.constraints = constraints;
        self
    }

    /// Format type for Rust syntax
    pub fn format_rust(&self) -> String {
        let mut result = String::new();

        if self.is_function_type {
            // Function type: |params| -> return_type or Fn(params) -> return_type
            result.push_str("impl Fn(");
            for (i, param) in self.function_params.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&param.format_rust());
            }
            result.push(')');
            if let Some(ref return_type) = self.function_return {
                result.push_str(" -> ");
                result.push_str(&return_type.format_rust());
            }
            return result;
        }

        result.push_str(&self.name);

        if !self.generic_args.is_empty() {
            result.push('<');
            for (i, arg) in self.generic_args.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&arg.format_rust());
            }
            result.push('>');
        }

        if self.is_nullable {
            result = format!("Option<{}>", result);
        }

        result
    }

    /// Format type for TypeScript syntax
    pub fn format_typescript(&self) -> String {
        let mut result = String::new();

        if self.is_function_type {
            // Function type: (params) => return_type
            let function_sig = {
                let mut func_result = String::new();
                func_result.push('(');
                for (i, param) in self.function_params.iter().enumerate() {
                    if i > 0 {
                        func_result.push_str(", ");
                    }
                    func_result.push_str(&format!("arg{}: {}", i, param.format_typescript()));
                }
                func_result.push_str(") => ");
                if let Some(ref return_type) = self.function_return {
                    func_result.push_str(&return_type.format_typescript());
                } else {
                    func_result.push_str("void");
                }
                func_result
            };
            
            // If this is an array of functions, wrap in parentheses
            if self.is_array {
                result.push('(');
                result.push_str(&function_sig);
                result.push(')');
                for _ in 0..self.array_dimensions {
                    result.push_str("[]");
                }
            } else {
                result.push_str(&function_sig);
            }
            
            if self.is_nullable {
                result.push_str(" | null");
            }
            
            return result;
        }

        result.push_str(&self.name);

        if !self.generic_args.is_empty() {
            result.push('<');
            for (i, arg) in self.generic_args.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&arg.format_typescript());
            }
            result.push('>');
        }

        if self.is_array {
            for _ in 0..self.array_dimensions {
                result.push_str("[]");
            }
        }

        if self.is_nullable {
            result.push_str(" | null");
        }

        result
    }

    /// Format type for Python syntax
    pub fn format_python(&self) -> String {
        let mut result = String::new();

        if self.is_function_type {
            // Function type: Callable[[params], return_type]
            result.push_str("Callable[[");
            for (i, param) in self.function_params.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&param.format_python());
            }
            result.push_str("], ");
            if let Some(ref return_type) = self.function_return {
                result.push_str(&return_type.format_python());
            } else {
                result.push_str("None");
            }
            result.push(']');
            return result;
        }

        result.push_str(&self.name);

        if !self.generic_args.is_empty() {
            result.push('[');
            for (i, arg) in self.generic_args.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&arg.format_python());
            }
            result.push(']');
        }

        if self.is_nullable {
            result = format!("Optional[{}]", result);
        }

        result
    }
}

/// Generic parameter with bounds and constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericParameter {
    /// Parameter name
    pub name: String,
    /// Type bounds (e.g., "Clone + Send" in Rust)
    pub bounds: Vec<String>,
    /// Default type (if any)
    pub default_type: Option<String>,
    /// Variance (covariant, contravariant, invariant)
    pub variance: Option<String>,
}

impl GenericParameter {
    /// Create a new generic parameter
    pub fn new(name: String) -> Self {
        Self {
            name,
            bounds: Vec::new(),
            default_type: None,
            variance: None,
        }
    }

    /// Add bounds to the generic parameter
    pub fn with_bounds(mut self, bounds: Vec<String>) -> Self {
        self.bounds = bounds;
        self
    }

    /// Set default type
    pub fn with_default(mut self, default_type: String) -> Self {
        self.default_type = Some(default_type);
        self
    }

    /// Format for Rust syntax
    pub fn format_rust(&self) -> String {
        let mut result = self.name.clone();
        if !self.bounds.is_empty() {
            result.push_str(": ");
            result.push_str(&self.bounds.join(" + "));
        }
        if let Some(ref default) = self.default_type {
            result.push_str(" = ");
            result.push_str(default);
        }
        result
    }

    /// Format for TypeScript syntax
    pub fn format_typescript(&self) -> String {
        let mut result = self.name.clone();
        if !self.bounds.is_empty() {
            result.push_str(" extends ");
            result.push_str(&self.bounds.join(" & "));
        }
        if let Some(ref default) = self.default_type {
            result.push_str(" = ");
            result.push_str(default);
        }
        result
    }
}

/// Function/method parameter with complete information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name
    pub name: String,
    /// Parameter type information
    pub type_info: Option<TypeInfo>,
    /// Default value expression
    pub default_value: Option<String>,
    /// Whether the parameter is optional
    pub is_optional: bool,
    /// Whether the parameter is variadic (rest/spread)
    pub is_variadic: bool,
    /// Whether the parameter is mutable
    pub is_mutable: bool,
    /// Parameter modifiers
    pub modifiers: Vec<Modifier>,
    /// Parameter attributes or decorators
    pub attributes: Vec<String>,
}

impl Parameter {
    /// Create a new parameter
    pub fn new(name: String) -> Self {
        Self {
            name,
            type_info: None,
            default_value: None,
            is_optional: false,
            is_variadic: false,
            is_mutable: false,
            modifiers: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Set the type information
    pub fn with_type(mut self, type_info: TypeInfo) -> Self {
        self.type_info = Some(type_info);
        self
    }

    /// Set default value
    pub fn with_default(mut self, default_value: String) -> Self {
        self.default_value = Some(default_value);
        self.is_optional = true;
        self
    }

    /// Make parameter optional
    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    /// Make parameter variadic
    pub fn variadic(mut self) -> Self {
        self.is_variadic = true;
        self
    }

    /// Make parameter mutable
    pub fn mutable(mut self) -> Self {
        self.is_mutable = true;
        self
    }

    /// Add modifiers
    pub fn with_modifiers(mut self, modifiers: Vec<Modifier>) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Format parameter for Rust syntax
    pub fn format_rust(&self) -> String {
        let mut result = String::new();

        // Add modifiers
        for modifier in &self.modifiers {
            let mod_str = modifier.as_rust_str();
            if !mod_str.is_empty() {
                result.push_str(mod_str);
                result.push(' ');
            }
        }

        if self.is_mutable {
            result.push_str("mut ");
        }

        result.push_str(&self.name);
        result.push_str(": ");

        if let Some(ref type_info) = self.type_info {
            result.push_str(&type_info.format_rust());
        } else {
            result.push_str("_");
        }

        result
    }

    /// Format parameter for TypeScript syntax
    pub fn format_typescript(&self) -> String {
        let mut result = String::new();

        if self.is_variadic {
            result.push_str("...");
        }

        result.push_str(&self.name);

        if self.is_optional {
            result.push('?');
        }

        result.push_str(": ");

        if let Some(ref type_info) = self.type_info {
            result.push_str(&type_info.format_typescript());
        } else {
            result.push_str("any");
        }

        if let Some(ref default) = self.default_value {
            result.push_str(" = ");
            result.push_str(default);
        }

        result
    }

    /// Format parameter for Python syntax
    pub fn format_python(&self) -> String {
        let mut result = String::new();

        if self.is_variadic {
            if self.name == "kwargs" || self.modifiers.contains(&Modifier::Ref) {
                result.push_str("**");
            } else {
                result.push('*');
            }
        }

        result.push_str(&self.name);

        if let Some(ref type_info) = self.type_info {
            result.push_str(": ");
            result.push_str(&type_info.format_python());
        }

        if let Some(ref default) = self.default_value {
            result.push_str(" = ");
            result.push_str(default);
        }

        result
    }
    
    /// Format parameter for Dart syntax
    pub fn format_dart(&self) -> String {
        let mut result = String::new();

        // In Dart, type comes before name
        if let Some(ref type_info) = self.type_info {
            let formatted_type = type_info.format_typescript(); // Dart types are similar to TS
            result.push_str(&formatted_type);
            if self.is_optional {
                result.push('?'); // Add ? for optional parameters in Dart
            }
            result.push(' ');
        }

        result.push_str(&self.name);

        if let Some(ref default) = self.default_value {
            result.push_str(" = ");
            result.push_str(default);
        }

        result
    }
}

/// Complete function/method signature with all details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// Function name
    pub name: String,
    /// Function parameters
    pub parameters: Vec<Parameter>,
    /// Return type information
    pub return_type: Option<TypeInfo>,
    /// Generic parameters
    pub generic_parameters: Vec<GenericParameter>,
    /// Function modifiers
    pub modifiers: Vec<Modifier>,
    /// Whether this is an async function
    pub is_async: bool,
    /// Whether this is a generator function
    pub is_generator: bool,
    /// Whether this is a constructor
    pub is_constructor: bool,
    /// Function attributes or decorators
    pub attributes: Vec<String>,
    /// Source language
    pub language: Language,
    /// Raw signature as extracted from source
    pub raw_signature: String,
}

impl Signature {
    /// Create a new signature
    pub fn new(name: String, language: Language) -> Self {
        Self {
            name,
            parameters: Vec::new(),
            return_type: None,
            generic_parameters: Vec::new(),
            modifiers: Vec::new(),
            is_async: false,
            is_generator: false,
            is_constructor: false,
            attributes: Vec::new(),
            language,
            raw_signature: String::new(),
        }
    }

    /// Add a parameter
    pub fn with_parameter(mut self, parameter: Parameter) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Set return type
    pub fn with_return_type(mut self, return_type: TypeInfo) -> Self {
        self.return_type = Some(return_type);
        self
    }

    /// Add generic parameter
    pub fn with_generic(mut self, generic: GenericParameter) -> Self {
        self.generic_parameters.push(generic);
        self
    }

    /// Add modifiers
    pub fn with_modifiers(mut self, modifiers: Vec<Modifier>) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Mark as async
    pub fn async_function(mut self) -> Self {
        self.is_async = true;
        self
    }

    /// Mark as generator
    pub fn generator(mut self) -> Self {
        self.is_generator = true;
        self
    }

    /// Mark as constructor
    pub fn constructor(mut self) -> Self {
        self.is_constructor = true;
        self
    }

    /// Set raw signature
    pub fn with_raw_signature(mut self, raw: String) -> Self {
        self.raw_signature = raw;
        self
    }

    /// Format signature for the appropriate language
    pub fn format_for_language(&self, language: Language) -> String {
        match language {
            Language::Rust => self.format_rust_style(),
            Language::TypeScript => self.format_typescript_style(),
            Language::JavaScript => self.format_javascript_style(),
            Language::Dart => self.format_dart_style(),
            Language::Python => self.format_python_style(),
            Language::Unknown => self.raw_signature.clone(),
        }
    }

    /// Format signature in Rust style
    pub fn format_rust_style(&self) -> String {
        let mut result = String::new();

        // Add modifiers
        for modifier in &self.modifiers {
            let mod_str = modifier.as_rust_str();
            if !mod_str.is_empty() {
                result.push_str(mod_str);
                result.push(' ');
            }
        }

        // Add async keyword
        if self.is_async {
            result.push_str("async ");
        }

        // Add appropriate keyword based on context
        if self.is_constructor {
            result.push_str("new");
        } else {
            // For Rust, we need to determine the appropriate keyword
            // This is a bit of a hack, but we can check the raw signature
            if self.raw_signature.contains("struct ") {
                result.push_str("struct");
            } else if self.raw_signature.contains("enum ") {
                result.push_str("enum");
            } else if self.raw_signature.contains("trait ") {
                result.push_str("trait");
            } else if self.raw_signature.contains("type ") {
                result.push_str("type");
            } else {
                result.push_str("fn");
            }
        }
        result.push(' ');
        result.push_str(&self.name);

        // Add generic parameters
        if !self.generic_parameters.is_empty() {
            result.push('<');
            for (i, generic) in self.generic_parameters.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&generic.format_rust());
            }
            result.push('>');
        }

        // Add parameters
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&param.format_rust());
        }
        result.push(')');

        // Add return type
        if let Some(ref return_type) = self.return_type {
            result.push_str(" -> ");
            result.push_str(&return_type.format_rust());
        }

        result
    }

    /// Format signature in TypeScript style
    pub fn format_typescript_style(&self) -> String {
        // If we have a raw signature that looks like a class/interface/type definition, use it
        if !self.raw_signature.is_empty() && 
           (self.raw_signature.starts_with("class ") ||
            self.raw_signature.starts_with("interface ") ||
            self.raw_signature.starts_with("type ") ||
            self.raw_signature.starts_with("enum ") ||
            self.raw_signature.starts_with("namespace ") ||
            self.raw_signature.starts_with("module ") ||
            self.raw_signature.contains("=>")) {
            return self.raw_signature.clone();
        }

        let mut result = String::new();

        // Add access modifiers
        for modifier in &self.modifiers {
            match modifier {
                Modifier::Public | Modifier::Private | Modifier::Protected => {
                    result.push_str(modifier.as_typescript_str());
                    result.push(' ');
                }
                _ => {}
            }
        }

        // Add other modifiers (including async from modifiers)
        for modifier in &self.modifiers {
            match modifier {
                Modifier::Static | Modifier::Abstract | Modifier::Readonly | Modifier::Override | Modifier::Async => {
                    result.push_str(modifier.as_typescript_str());
                    result.push(' ');
                }
                _ => {}
            }
        }

        // Add async keyword if not already added from modifiers
        if self.is_async && !self.modifiers.contains(&Modifier::Async) {
            result.push_str("async ");
        }

        // Add function name (constructors don't have explicit names in TS)
        if !self.is_constructor {
            result.push_str(&self.name);
        } else {
            result.push_str("constructor");
        }

        // Add generic parameters
        if !self.generic_parameters.is_empty() {
            result.push('<');
            for (i, generic) in self.generic_parameters.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&generic.format_typescript());
            }
            result.push('>');
        }

        // Add parameters
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&param.format_typescript());
        }
        result.push(')');

        // Add return type
        if let Some(ref return_type) = self.return_type {
            result.push_str(": ");
            result.push_str(&return_type.format_typescript());
        } else if !self.is_constructor {
            result.push_str(": void");
        }

        result
    }

    /// Format signature in JavaScript style
    pub fn format_javascript_style(&self) -> String {
        let mut result = String::new();

        // Add async keyword
        if self.is_async {
            result.push_str("async ");
        }

        // Add function keyword or constructor
        if self.is_constructor {
            result.push_str("constructor");
        } else {
            result.push_str("function ");
            result.push_str(&self.name);
        }

        // Add parameters (no types in JS)
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            if param.is_variadic {
                result.push_str("...");
            }
            result.push_str(&param.name);
            if let Some(ref default) = param.default_value {
                result.push_str(" = ");
                result.push_str(default);
            }
        }
        result.push(')');

        result
    }

    /// Format signature in Python style
    pub fn format_python_style(&self) -> String {
        let mut result = String::new();

        // Add decorators/attributes
        for attr in &self.attributes {
            result.push('@');
            result.push_str(attr);
            result.push('\n');
        }
        
        // Add modifiers as decorators
        for modifier in &self.modifiers {
            let mod_str = modifier.as_python_str();
            if !mod_str.is_empty() {
                result.push_str(mod_str);
                result.push('\n');
            }
        }

        // Add async keyword
        if self.is_async {
            result.push_str("async ");
        }

        // Add def keyword
        result.push_str("def ");
        result.push_str(&self.name);

        // Add parameters
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&param.format_python());
        }
        result.push(')');

        // Add return type annotation
        if let Some(ref return_type) = self.return_type {
            result.push_str(" -> ");
            result.push_str(&return_type.format_python());
        }

        result
    }

    /// Format signature in Dart style
    pub fn format_dart_style(&self) -> String {
        let mut result = String::new();

        // Add modifiers
        for modifier in &self.modifiers {
            let mod_str = modifier.as_dart_str();
            if !mod_str.is_empty() {
                result.push_str(mod_str);
                result.push(' ');
            }
        }

        // Add return type (before function name in Dart)
        if let Some(ref return_type) = self.return_type {
            result.push_str(&return_type.format_typescript()); // Dart syntax is similar to TS
            result.push(' ');
        }

        // Add function name
        result.push_str(&self.name);

        // Add parameters
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&param.format_dart()); // Use Dart parameter formatting
        }
        result.push(')');

        result
    }
}

/// Trait for extracting comprehensive signatures from Tree-sitter nodes
pub trait SignatureExtractor {
    /// Extract function signature from a Tree-sitter node
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature>;

    /// Extract method signature from a Tree-sitter node
    fn extract_method_signature(&self, node: &Node, source: &str) -> Option<Signature>;

    /// Extract constructor signature from a Tree-sitter node
    fn extract_constructor_signature(&self, node: &Node, source: &str) -> Option<Signature>;

    /// Extract type signature (for type aliases, etc.)
    fn extract_type_signature(&self, node: &Node, source: &str) -> Option<Signature>;

    /// Parse type information from a Tree-sitter node
    fn parse_type_info(&self, node: &Node, source: &str) -> Option<TypeInfo>;

    /// Parse parameter information from a Tree-sitter node
    fn parse_parameter(&self, node: &Node, source: &str) -> Option<Parameter>;

    /// Parse generic parameters from a Tree-sitter node
    fn parse_generic_parameters(&self, node: &Node, source: &str) -> Vec<GenericParameter>;

    /// Parse modifiers from a Tree-sitter node
    fn parse_modifiers(&self, node: &Node, source: &str) -> Vec<Modifier>;
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_for_language(self.language.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_comprehensive_rust_signature_scenarios() {
        // Test complex generic function with bounds
        let signature = Signature::new("process_data".to_string(), Language::Rust)
            .with_modifiers(vec![Modifier::Public, Modifier::Async])
            .with_generic(
                GenericParameter::new("T".to_string())
                    .with_bounds(vec!["Clone".to_string(), "Send".to_string(), "Sync".to_string()])
            )
            .with_generic(
                GenericParameter::new("E".to_string())
                    .with_bounds(vec!["std::error::Error".to_string()])
            )
            .with_parameter(
                Parameter::new("items".to_string())
                    .with_type(TypeInfo::generic("Vec".to_string(), vec![TypeInfo::new("T".to_string())]))
            )
            .with_parameter(
                Parameter::new("processor".to_string())
                    .with_type(TypeInfo::function(
                        vec![TypeInfo::new("T".to_string())],
                        Some(TypeInfo::generic("Result".to_string(), vec![
                            TypeInfo::new("T".to_string()),
                            TypeInfo::new("E".to_string())
                        ]))
                    ))
            )
            .with_return_type(
                TypeInfo::generic("Result".to_string(), vec![
                    TypeInfo::generic("Vec".to_string(), vec![TypeInfo::new("T".to_string())]),
                    TypeInfo::new("E".to_string())
                ])
            )
            .with_raw_signature("fn process_data".to_string());

        let formatted = signature.format_rust_style();
        assert!(formatted.contains("pub async fn process_data"));
        assert!(formatted.contains("<T: Clone + Send + Sync, E: std::error::Error>"));
        assert!(formatted.contains("items: Vec<T>"));
        assert!(formatted.contains("processor: impl Fn(T) -> Result<T, E>"));
        assert!(formatted.contains("-> Result<Vec<T>, E>"));
    }

    #[test]
    fn test_comprehensive_typescript_signature_scenarios() {
        // Test complex TypeScript method with generics and optional parameters
        let signature = Signature::new("processAsync".to_string(), Language::TypeScript)
            .with_modifiers(vec![Modifier::Public, Modifier::Static, Modifier::Async])
            .with_generic(
                GenericParameter::new("T".to_string())
                    .with_bounds(vec!["Serializable".to_string()])
            )
            .with_generic(
                GenericParameter::new("U".to_string())
                    .with_bounds(vec!["Deserializable".to_string()])
                    .with_default("T".to_string())
            )
            .with_parameter(
                Parameter::new("data".to_string())
                    .with_type(TypeInfo::array(TypeInfo::new("T".to_string()), 1))
            )
            .with_parameter(
                Parameter::new("options".to_string())
                    .with_type(TypeInfo::new("ProcessOptions".to_string()))
                    .optional()
            )
            .with_parameter(
                Parameter::new("handlers".to_string())
                    .with_type(TypeInfo::array(
                        TypeInfo::function(
                            vec![TypeInfo::new("T".to_string())],
                            Some(TypeInfo::generic("Promise".to_string(), vec![TypeInfo::new("U".to_string())]))
                        ),
                        1
                    ))
                    .variadic()
            )
            .with_return_type(
                TypeInfo::generic("Promise".to_string(), vec![
                    TypeInfo::generic("ProcessResult".to_string(), vec![TypeInfo::new("U".to_string())])
                ])
            );

        let formatted = signature.format_typescript_style();
        assert!(formatted.contains("public static async processAsync"));
        assert!(formatted.contains("<T extends Serializable, U extends Deserializable = T>"));
        assert!(formatted.contains("data: T[]"));
        assert!(formatted.contains("options?: ProcessOptions"));
        assert!(formatted.contains("...handlers: ((arg0: T) => Promise<U>)[]"));
        assert!(formatted.contains(": Promise<ProcessResult<U>>"));
    }

    #[test]
    fn test_python_signature_with_type_hints() {
        // Test Python function with comprehensive type hints
        let signature = Signature::new("process_data".to_string(), Language::Python)
            .with_modifiers(vec![Modifier::Static])
            .with_parameter(
                Parameter::new("items".to_string())
                    .with_type(TypeInfo::generic("List".to_string(), vec![TypeInfo::new("T".to_string())]))
            )
            .with_parameter(
                Parameter::new("processor".to_string())
                    .with_type(TypeInfo::function(
                        vec![TypeInfo::new("T".to_string())],
                        Some(TypeInfo::generic("Awaitable".to_string(), vec![TypeInfo::new("T".to_string())]))
                    ))
            )
            .with_parameter(
                Parameter::new("args".to_string())
                    .with_type(TypeInfo::new("Any".to_string()))
                    .variadic()
            )
            .with_parameter(
                Parameter::new("timeout".to_string())
                    .with_type(TypeInfo::generic("Optional".to_string(), vec![TypeInfo::new("float".to_string())]))
                    .with_default("None".to_string())
            )
            .with_parameter(
                Parameter::new("kwargs".to_string())
                    .with_type(TypeInfo::generic("Dict".to_string(), vec![
                        TypeInfo::new("str".to_string()),
                        TypeInfo::new("Any".to_string())
                    ]))
                    .variadic()
                    .with_modifiers(vec![Modifier::Ref])
            )
            .with_return_type(
                TypeInfo::generic("AsyncIterator".to_string(), vec![TypeInfo::new("T".to_string())])
            )
            .async_function();

        let formatted = signature.format_python_style();
        assert!(formatted.contains("@staticmethod"));
        assert!(formatted.contains("async def process_data"));
        assert!(formatted.contains("items: List[T]"));
        assert!(formatted.contains("processor: Callable[[T], Awaitable[T]]"));
        assert!(formatted.contains("*args: Any"));
        assert!(formatted.contains("timeout: Optional[float] = None"));
        assert!(formatted.contains("**kwargs: Dict[str, Any]"));
        assert!(formatted.contains("-> AsyncIterator[T]"));
    }

    #[test]
    fn test_dart_signature_with_named_parameters() {
        // Test Dart method with named and positional parameters
        let signature = Signature::new("processData".to_string(), Language::Dart)
            .with_modifiers(vec![Modifier::Static, Modifier::Final])
            .with_parameter(
                Parameter::new("data".to_string())
                    .with_type(TypeInfo::generic("List".to_string(), vec![TypeInfo::new("T".to_string())]))
            )
            .with_parameter(
                Parameter::new("required".to_string())
                    .with_type(TypeInfo::new("String".to_string()))
            )
            .with_parameter(
                Parameter::new("optional".to_string())
                    .with_type(TypeInfo::new("int".to_string()))
                    .optional()
            )
            .with_return_type(
                TypeInfo::generic("Future".to_string(), vec![
                    TypeInfo::generic("Result".to_string(), vec![
                        TypeInfo::new("T".to_string()),
                        TypeInfo::new("Exception".to_string())
                    ])
                ])
            );

        let formatted = signature.format_dart_style();
        assert!(formatted.contains("static final"));
        assert!(formatted.contains("Future<Result<T, Exception>> processData"));
        assert!(formatted.contains("List<T> data"));
        assert!(formatted.contains("String required"));
        assert!(formatted.contains("int? optional"));
    }

    #[test]
    fn test_complex_reference_and_array_types() {
        // Test Rust references and arrays
        let ref_type = TypeInfo::new("&mut Vec<T>".to_string());
        assert_eq!(ref_type.format_rust(), "&mut Vec<T>");

        let array_type = TypeInfo::array(
            TypeInfo::generic("HashMap".to_string(), vec![
                TypeInfo::new("String".to_string()),
                TypeInfo::new("i32".to_string())
            ]),
            2
        );
        assert!(array_type.is_array);
        assert_eq!(array_type.array_dimensions, 2);

        // Test nullable types
        let nullable_type = TypeInfo::new("String".to_string()).nullable();
        assert!(nullable_type.is_nullable);
        assert_eq!(nullable_type.format_rust(), "Option<String>");
        assert!(nullable_type.format_typescript().contains("| null"));
        assert_eq!(nullable_type.format_python(), "Optional[String]");
    }

    #[test]
    fn test_function_type_signatures() {
        // Test complex function types
        let closure_type = TypeInfo::function(
            vec![
                TypeInfo::new("i32".to_string()),
                TypeInfo::generic("Vec".to_string(), vec![TypeInfo::new("String".to_string())])
            ],
            Some(TypeInfo::generic("Result".to_string(), vec![
                TypeInfo::new("bool".to_string()),
                TypeInfo::new("Error".to_string())
            ]))
        );

        let rust_format = closure_type.format_rust();
        assert!(rust_format.contains("impl Fn("));
        assert!(rust_format.contains("i32, Vec<String>"));
        assert!(rust_format.contains("-> Result<bool, Error>"));

        let ts_format = closure_type.format_typescript();
        assert!(ts_format.contains("(arg0: i32, arg1: Vec<String>) => Result<bool, Error>"));

        let python_format = closure_type.format_python();
        assert!(python_format.contains("Callable[[i32, Vec[String]], Result[bool, Error]]"));
    }

    #[test]
    fn test_generic_parameter_bounds_and_defaults() {
        // Test complex generic parameter scenarios
        let bounded_generic = GenericParameter::new("T".to_string())
            .with_bounds(vec![
                "Clone".to_string(),
                "Send".to_string(),
                "Sync".to_string(),
                "'static".to_string()
            ]);

        let rust_format = bounded_generic.format_rust();
        assert_eq!(rust_format, "T: Clone + Send + Sync + 'static");

        let defaulted_generic = GenericParameter::new("U".to_string())
            .with_bounds(vec!["Serialize".to_string()])
            .with_default("String".to_string());

        let ts_format = defaulted_generic.format_typescript();
        assert_eq!(ts_format, "U extends Serialize = String");
    }

    #[test]
    fn test_parameter_modifiers_and_attributes() {
        // Test various parameter modifiers
        let mutable_param = Parameter::new("value".to_string())
            .with_type(TypeInfo::new("i32".to_string()))
            .mutable()
            .with_modifiers(vec![Modifier::Mut]);

        let rust_format = mutable_param.format_rust();
        assert!(rust_format.contains("mut value: i32"));

        let variadic_param = Parameter::new("args".to_string())
            .with_type(TypeInfo::new("string".to_string()))
            .variadic();

        let ts_format = variadic_param.format_typescript();
        assert!(ts_format.contains("...args: string"));

        let python_format = variadic_param.format_python();
        assert!(python_format.contains("*args: string"));
    }

    #[test]
    fn test_signature_display_formatting() {
        // Test that Display trait works correctly for different languages
        let rust_sig = Signature::new("test".to_string(), Language::Rust)
            .with_parameter(Parameter::new("x".to_string()).with_type(TypeInfo::new("i32".to_string())))
            .with_raw_signature("fn test".to_string());

        let display_output = format!("{}", rust_sig);
        assert!(display_output.contains("fn test"));
        assert!(display_output.contains("x: i32"));

        let ts_sig = Signature::new("test".to_string(), Language::TypeScript)
            .with_parameter(Parameter::new("x".to_string()).with_type(TypeInfo::new("number".to_string())));

        let ts_display = format!("{}", ts_sig);
        assert!(ts_display.contains("test"));
        assert!(ts_display.contains("x: number"));
    }

    #[test]
    fn test_type_info_creation() {
        let simple_type = TypeInfo::new("String".to_string());
        assert_eq!(simple_type.name, "String");
        assert!(!simple_type.is_nullable);
        assert!(!simple_type.is_array);

        let generic_type = TypeInfo::generic(
            "Vec".to_string(),
            vec![TypeInfo::new("i32".to_string())],
        );
        assert_eq!(generic_type.name, "Vec");
        assert_eq!(generic_type.generic_args.len(), 1);
        assert_eq!(generic_type.generic_args[0].name, "i32");
    }

    #[test]
    fn test_parameter_creation() {
        let param = Parameter::new("value".to_string())
            .with_type(TypeInfo::new("i32".to_string()))
            .with_default("0".to_string());

        assert_eq!(param.name, "value");
        assert!(param.type_info.is_some());
        assert_eq!(param.type_info.unwrap().name, "i32");
        assert!(param.is_optional);
        assert_eq!(param.default_value.unwrap(), "0");
    }

    #[test]
    fn test_signature_creation() {
        let signature = Signature::new("test_function".to_string(), Language::Rust)
            .with_parameter(
                Parameter::new("x".to_string())
                    .with_type(TypeInfo::new("i32".to_string()))
            )
            .with_return_type(TypeInfo::new("String".to_string()))
            .async_function();

        assert_eq!(signature.name, "test_function");
        assert_eq!(signature.parameters.len(), 1);
        assert!(signature.return_type.is_some());
        assert!(signature.is_async);
    }

    #[test]
    fn test_rust_signature_formatting() {
        let signature = Signature::new("test_fn".to_string(), Language::Rust)
            .with_modifiers(vec![Modifier::Public])
            .with_parameter(
                Parameter::new("x".to_string())
                    .with_type(TypeInfo::new("i32".to_string()))
            )
            .with_return_type(TypeInfo::new("String".to_string()))
            .async_function();

        let formatted = signature.format_rust_style();
        assert!(formatted.contains("pub"));
        assert!(formatted.contains("async"));
        assert!(formatted.contains("fn test_fn"));
        assert!(formatted.contains("x: i32"));
        assert!(formatted.contains("-> String"));
    }

    #[test]
    fn test_typescript_signature_formatting() {
        let signature = Signature::new("testFunction".to_string(), Language::TypeScript)
            .with_modifiers(vec![Modifier::Public, Modifier::Async])
            .with_parameter(
                Parameter::new("x".to_string())
                    .with_type(TypeInfo::new("number".to_string()))
                    .optional()
            )
            .with_return_type(TypeInfo::new("Promise<string>".to_string()));

        let formatted = signature.format_typescript_style();
        assert!(formatted.contains("public"));
        assert!(formatted.contains("async"));
        assert!(formatted.contains("testFunction"));
        assert!(formatted.contains("x?: number"));
        assert!(formatted.contains(": Promise<string>"));
    }

    #[test]
    fn test_complex_generic_signature() {
        let generic_param = GenericParameter::new("T".to_string())
            .with_bounds(vec!["Clone".to_string(), "Send".to_string()]);

        let signature = Signature::new("process".to_string(), Language::Rust)
            .with_generic(generic_param)
            .with_parameter(
                Parameter::new("items".to_string())
                    .with_type(TypeInfo::generic("Vec".to_string(), vec![TypeInfo::new("T".to_string())]))
            )
            .with_return_type(TypeInfo::generic("Result".to_string(), vec![
                TypeInfo::new("T".to_string()),
                TypeInfo::new("Error".to_string())
            ]));

        let formatted = signature.format_rust_style();
        assert!(formatted.contains("fn process<T: Clone + Send>"));
        assert!(formatted.contains("items: Vec<T>"));
        assert!(formatted.contains("-> Result<T, Error>"));
    }

    #[test]
    fn test_function_type() {
        let function_type = TypeInfo::function(
            vec![TypeInfo::new("i32".to_string()), TypeInfo::new("i32".to_string())],
            Some(TypeInfo::new("i32".to_string()))
        );

        let rust_format = function_type.format_rust();
        assert!(rust_format.contains("impl Fn("));
        assert!(rust_format.contains("-> i32"));

        let ts_format = function_type.format_typescript();
        assert!(ts_format.contains("(arg0: i32, arg1: i32) => i32"));
    }
}