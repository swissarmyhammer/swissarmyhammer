# OUTLINE_000252: Signature Extraction Enhancement

Refer to ./specification/outline_tool.md

## Summary

Enhance the outline tool with comprehensive signature extraction capabilities for all supported languages, generating accurate function/method signatures with complete type information, parameter details, and return types.

## Context

While basic signature extraction was implemented in the language-specific extractors, this step focuses on creating a unified, comprehensive signature extraction system that handles complex scenarios across all languages and provides consistent, detailed signatures.

## Requirements

### 1. Unified Signature Interface
- Consistent signature format across all languages
- Language-specific formatting while maintaining consistency
- Comprehensive parameter information extraction
- Return type and generic parameter handling
- Visibility and modifier information

### 2. Advanced Signature Features
- **Generic parameters**: Type parameters with bounds and constraints
- **Complex types**: Union types, intersection types, function types
- **Default parameters**: Default values and optional parameters
- **Variadic parameters**: Rest parameters, varargs, *args/**kwargs
- **Async signatures**: Async functions, generators, coroutines
- **Operator overloads**: Custom operators and special methods

### 3. Cross-Language Signature Normalization
- Consistent representation of similar concepts
- Language-specific syntax preservation
- Readable formatting for human consumption
- Parseable format for tooling

## Technical Details

### Signature Extractor Framework
```rust
pub trait SignatureExtractor {
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature>;
    fn extract_method_signature(&self, node: &Node, source: &str) -> Option<Signature>;
    fn extract_constructor_signature(&self, node: &Node, source: &str) -> Option<Signature>;
    fn extract_type_signature(&self, node: &Node, source: &str) -> Option<Signature>;
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeInfo>,
    pub generic_parameters: Vec<GenericParameter>,
    pub modifiers: Vec<Modifier>,
    pub is_async: bool,
    pub is_generator: bool,
    pub language: Language,
    pub raw_signature: String,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub type_info: Option<TypeInfo>,
    pub default_value: Option<String>,
    pub is_optional: bool,
    pub is_variadic: bool,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub generic_args: Vec<TypeInfo>,
    pub is_nullable: bool,
    pub is_array: bool,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GenericParameter {
    pub name: String,
    pub bounds: Vec<String>,
    pub default_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    Public,
    Private,
    Protected,
    Static,
    Abstract,
    Final,
    Async,
    Const,
    Readonly,
    Override,
    Virtual,
    Unsafe,
}
```

### Language-Specific Signature Extraction

#### Rust Signature Extraction
```rust
impl SignatureExtractor for RustExtractor {
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let mut signature = Signature::new(Language::Rust);
        
        // Extract visibility
        if let Some(vis_node) = node.child_by_field_name("visibility") {
            signature.modifiers.push(Self::parse_rust_visibility(vis_node, source));
        }
        
        // Extract function name
        if let Some(name_node) = node.child_by_field_name("name") {
            signature.name = name_node.utf8_text(source).unwrap().to_string();
        }
        
        // Extract generic parameters
        if let Some(generics_node) = node.child_by_field_name("type_parameters") {
            signature.generic_parameters = Self::parse_rust_generics(generics_node, source);
        }
        
        // Extract parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            signature.parameters = Self::parse_rust_parameters(params_node, source);
        }
        
        // Extract return type
        if let Some(return_node) = node.child_by_field_name("return_type") {
            signature.return_type = Some(Self::parse_rust_type(return_node, source));
        }
        
        // Check for async
        signature.is_async = source[node.start_byte()..node.end_byte()].contains("async");
        
        // Generate raw signature
        signature.raw_signature = Self::format_rust_signature(&signature);
        
        Some(signature)
    }
}
```

#### TypeScript Signature Extraction
```rust
impl SignatureExtractor for TypeScriptExtractor {
    fn extract_function_signature(&self, node: &Node, source: &str) -> Option<Signature> {
        let mut signature = Signature::new(Language::TypeScript);
        
        // Extract access modifiers
        for modifier in Self::extract_ts_modifiers(node, source) {
            signature.modifiers.push(modifier);
        }
        
        // Extract function name
        signature.name = Self::extract_ts_function_name(node, source)?;
        
        // Extract generic parameters
        if let Some(generics_node) = node.child_by_field_name("type_parameters") {
            signature.generic_parameters = Self::parse_ts_generics(generics_node, source);
        }
        
        // Extract parameters with optional and rest parameters
        if let Some(params_node) = node.child_by_field_name("parameters") {
            signature.parameters = Self::parse_ts_parameters(params_node, source);
        }
        
        // Extract return type annotation
        if let Some(return_node) = node.child_by_field_name("return_type") {
            signature.return_type = Some(Self::parse_ts_type(return_node, source));
        }
        
        // Check for async
        signature.is_async = Self::is_async_function(node, source);
        
        signature.raw_signature = Self::format_ts_signature(&signature);
        
        Some(signature)
    }
}
```

### Signature Formatting
```rust
impl Signature {
    pub fn format_for_language(&self, language: Language) -> String {
        match language {
            Language::Rust => self.format_rust_style(),
            Language::TypeScript => self.format_typescript_style(),
            Language::JavaScript => self.format_javascript_style(),
            Language::Dart => self.format_dart_style(),
            Language::Python => self.format_python_style(),
        }
    }
    
    fn format_rust_style(&self) -> String {
        let mut result = String::new();
        
        // Add modifiers
        for modifier in &self.modifiers {
            result.push_str(&format!("{} ", modifier.as_rust_str()));
        }
        
        // Add async keyword
        if self.is_async {
            result.push_str("async ");
        }
        
        result.push_str("fn ");
        result.push_str(&self.name);
        
        // Add generic parameters
        if !self.generic_parameters.is_empty() {
            result.push('<');
            for (i, generic) in self.generic_parameters.iter().enumerate() {
                if i > 0 { result.push_str(", "); }
                result.push_str(&generic.format_rust());
            }
            result.push('>');
        }
        
        // Add parameters
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
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
    
    fn format_typescript_style(&self) -> String {
        let mut result = String::new();
        
        // Add access modifiers
        for modifier in &self.modifiers {
            if matches!(modifier, Modifier::Public | Modifier::Private | Modifier::Protected) {
                result.push_str(&format!("{} ", modifier.as_typescript_str()));
            }
        }
        
        // Add static/abstract modifiers
        for modifier in &self.modifiers {
            if matches!(modifier, Modifier::Static | Modifier::Abstract) {
                result.push_str(&format!("{} ", modifier.as_typescript_str()));
            }
        }
        
        // Add async keyword
        if self.is_async {
            result.push_str("async ");
        }
        
        result.push_str(&self.name);
        
        // Add generic parameters
        if !self.generic_parameters.is_empty() {
            result.push('<');
            for (i, generic) in self.generic_parameters.iter().enumerate() {
                if i > 0 { result.push_str(", "); }
                result.push_str(&generic.format_typescript());
            }
            result.push('>');
        }
        
        // Add parameters
        result.push('(');
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 { result.push_str(", "); }
            result.push_str(&param.format_typescript());
        }
        result.push(')');
        
        // Add return type
        if let Some(ref return_type) = self.return_type {
            result.push_str(": ");
            result.push_str(&return_type.format_typescript());
        }
        
        result
    }
}
```

## Implementation Steps

1. Create `src/outline/signature.rs` module with core signature types
2. Implement unified signature extraction framework
3. Enhance Rust signature extraction with generics and lifetimes
4. Enhance TypeScript signature extraction with complex types
5. Enhance JavaScript signature extraction with JSDoc types
6. Enhance Dart signature extraction with named parameters
7. Enhance Python signature extraction with type hints
8. Implement signature formatting for each language
9. Add signature validation and normalization
10. Create comprehensive test suite with complex signatures
11. Add performance optimizations for signature extraction
12. Integrate enhanced signatures with outline generation

## Testing Requirements

### Comprehensive Signature Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rust_generic_function_signature() {
        let source = r#"
            pub async fn process_data<T, E>(
                items: Vec<T>, 
                processor: impl Fn(T) -> Result<T, E>
            ) -> Result<Vec<T>, E> 
            where 
                T: Clone + Send,
                E: std::error::Error
        "#;
        
        // Test comprehensive signature extraction
    }
    
    #[test]
    fn test_typescript_complex_signature() {
        let source = r#"
            public async function processData<T extends Serializable>(
                items: readonly T[],
                options?: ProcessOptions,
                ...processors: ((item: T) => Promise<T>)[]
            ): Promise<ProcessResult<T>>
        "#;
        
        // Test TypeScript signature extraction
    }
    
    #[test]
    fn test_python_typed_signature() {
        let source = r#"
            async def process_data(
                items: List[T],
                processor: Callable[[T], Awaitable[T]],
                *args: Any,
                timeout: Optional[float] = None,
                **kwargs: Dict[str, Any]
            ) -> AsyncIterator[T]:
        "#;
        
        // Test Python signature with type hints
    }
}
```

### Integration Testing
- Test signature extraction with real codebases
- Verify signature accuracy for complex generic scenarios
- Test performance with large numbers of signatures
- Validate cross-language consistency

## Performance Considerations

- Cache parsed type information to avoid re-parsing
- Optimize Tree-sitter query execution for signature extraction
- Minimize string allocations during signature building
- Use efficient data structures for parameter and type storage

## Error Handling

- Graceful handling of malformed signatures
- Fallback to partial signature extraction when possible
- Clear error messages for signature parsing failures
- Recovery from incomplete type information

## Integration Points

### With Language Extractors
- Enhance existing language extractors with improved signature extraction
- Maintain backward compatibility with existing signature formats
- Integrate signature validation into extraction pipeline

### With YAML Formatter
- Provide formatted signatures for YAML output
- Support configurable signature formatting options
- Handle long signatures with wrapping or truncation

## Success Criteria

- Accurate signature extraction for all supported language features
- Consistent signature representation across languages
- Comprehensive handling of generic parameters and complex types
- Clean, readable signature formatting
- Performance suitable for large codebases
- Comprehensive test coverage with complex real-world examples
- Graceful error handling and recovery

## Dependencies

- Enhanced Tree-sitter queries for each language
- Core outline types and structures
- Language-specific parsing utilities
- String formatting and validation utilities

## Notes

Signature extraction is a critical feature for code navigation and understanding. The implementation should prioritize accuracy and completeness while maintaining good performance. Consider providing different levels of signature detail (brief vs. full) to accommodate different use cases.

## Proposed Solution

After analyzing the current codebase, I'll implement a comprehensive signature extraction system with the following approach:

### 1. Core Architecture
- Create a new `src/outline/signature.rs` module with unified signature types
- Implement a `SignatureExtractor` trait for consistent signature extraction across languages
- Define comprehensive data structures for signatures, parameters, types, and generics

### 2. Current State Analysis
The existing signature extraction is basic - each language extractor has simple signature building methods that concatenate strings. The current implementation lacks:
- Detailed parameter information (optional, variadic, default values)
- Complex type handling (union types, generics with bounds)
- Consistent cross-language representation
- Structured parameter and return type information

### 3. Implementation Steps
1. **Signature Data Structures**: Create `Signature`, `Parameter`, `TypeInfo`, `GenericParameter`, and `Modifier` structs
2. **SignatureExtractor Trait**: Define interface for extracting function, method, constructor, and type signatures
3. **Language-Specific Enhancement**: Enhance each language extractor with advanced signature extraction
4. **Signature Formatting**: Implement language-specific signature formatting while maintaining consistency
5. **Integration**: Integrate with existing outline generation pipeline
6. **Testing**: Create comprehensive tests covering complex scenarios

### 4. Key Features
- **Generic Parameters**: Full support for type parameters with bounds and constraints
- **Complex Types**: Union types, intersection types, function types, nullable types
- **Parameter Details**: Default values, optional parameters, variadic parameters
- **Visibility and Modifiers**: Complete modifier support (async, static, abstract, etc.)
- **Cross-Language Consistency**: Uniform representation while preserving language-specific syntax

### 5. Benefits
- Accurate signature representation for all supported languages
- Enhanced code navigation and understanding
- Foundation for future IDE integrations
- Consistent API for tooling consumption
- Improved YAML output formatting

The implementation will prioritize accuracy and completeness while maintaining good performance and backward compatibility with existing outline functionality.