# OUTLINE_000246: Rust Language Support

Refer to ./specification/outline_tool.md

## Summary

Implement comprehensive Rust language support for the outline tool, including extraction of structs, enums, traits, impls, functions, methods, modules, and Rustdoc comments with proper visibility and signature information.

## Context

Rust has rich type system features and specific documentation patterns that need specialized handling. This includes understanding visibility modifiers, generic parameters, lifetime annotations, trait bounds, and associated types.

## Requirements

### 1. Rust Symbol Types
Support all major Rust constructs:
- **Structs**: Regular, tuple, and unit structs with fields
- **Enums**: Enums with variants and associated data
- **Traits**: Trait definitions with associated types and methods
- **Impls**: Implementation blocks (inherent and trait implementations)
- **Functions**: Free functions with full signatures
- **Methods**: Associated functions and methods in impl blocks
- **Modules**: Module declarations and definitions
- **Constants**: Const and static declarations
- **Type aliases**: Type definitions

### 2. Rust-Specific Features
- **Visibility**: pub, pub(crate), pub(super), pub(in path)
- **Generics**: Type parameters with bounds
- **Lifetimes**: Lifetime parameters and annotations
- **Async**: Async functions and methods
- **Unsafe**: Unsafe functions and blocks
- **Attributes**: Important attributes like #[derive], #[cfg]

### 3. Rustdoc Comment Extraction
- **Doc comments**: `///` and `//!` patterns
- **Inner docs**: Module-level documentation
- **Example code**: Code blocks in documentation
- **Link parsing**: Intra-doc links and references

## Technical Details

### Rust Extractor Implementation
```rust
pub struct RustExtractor {
    query: Query,
}

impl SymbolExtractor for RustExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<OutlineNode> {
        // Implementation for Rust-specific symbol extraction
    }
}
```

### Tree-sitter Queries for Rust
Define comprehensive queries to capture:

```scheme
; Struct definitions
(struct_item
  name: (type_identifier) @struct.name
  body: (field_declaration_list
    (field_declaration
      name: (field_identifier) @field.name
      type: (_) @field.type))?) @struct.body

; Function definitions  
(function_item
  (visibility_modifier)? @function.visibility
  name: (identifier) @function.name
  parameters: (parameters) @function.params
  return_type: (type_annotation (type_identifier) @function.return)?)

; Impl blocks
(impl_item
  trait: (type_identifier)? @impl.trait
  type: (type_identifier) @impl.type
  body: (declaration_list) @impl.body)

; Documentation comments
(line_comment) @doc.comment
```

### Rust Signature Extraction
Generate accurate signatures including:
```rust
pub fn extract_rust_signature(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "function_item" => extract_function_signature(node, source),
        "struct_item" => extract_struct_signature(node, source),
        "enum_item" => extract_enum_signature(node, source),
        "trait_item" => extract_trait_signature(node, source),
        "impl_item" => extract_impl_signature(node, source),
        _ => None,
    }
}
```

Example signatures:
- `pub fn process<T: Clone>(data: T) -> Result<T, ProcessError>`
- `pub struct Config<'a> { name: &'a str, timeout: Duration }`
- `impl<T> Display for Wrapper<T> where T: Display`
- `pub trait Repository: Send + Sync { type Item; }`

## Implementation Steps

1. Create `src/outline/extractors/rust.rs`
2. Define comprehensive Tree-sitter queries for all Rust constructs
3. Implement symbol extraction for each construct type
4. Add signature generation for functions, methods, types
5. Implement Rustdoc comment extraction and parsing
6. Handle visibility modifiers and attributes
7. Add support for generic parameters and lifetimes
8. Create comprehensive unit tests with real Rust code

## Testing Requirements

### Test Cases
Create test files covering:
- **Basic structures**: Simple structs, enums, functions
- **Generic types**: Structs and functions with type parameters
- **Trait system**: Traits, implementations, associated types
- **Module system**: Module declarations, use statements
- **Async/await**: Async functions and blocks
- **Unsafe code**: Unsafe functions and implementations
- **Macros**: Macro definitions and invocations
- **Documentation**: Various doc comment patterns

### Sample Test Code
```rust
/// Configuration for the application
pub struct Config<'a> {
    /// Application name
    pub name: &'a str,
    /// Timeout duration
    pub timeout: Duration,
}

impl<'a> Config<'a> {
    /// Create a new configuration
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Trait for processing data
pub trait Processor: Send + Sync {
    type Input;
    type Output;
    
    /// Process the input data
    async fn process(&self, input: Self::Input) -> Result<Self::Output, ProcessError>;
}
```

Expected outline structure:
```yaml
Config:
  kind: struct
  line: 2
  signature: "pub struct Config<'a>"
  doc: "Configuration for the application"
  children:
    - name: "name"
      kind: field
      type: "&'a str"
      doc: "Application name"
    - name: "new"
      kind: method
      signature: "pub fn new(name: &'a str) -> Self"
      doc: "Create a new configuration"
```

## Integration Points

### With Core Parser
- Implement `SymbolExtractor` trait for Rust
- Register Rust extractor with language detection
- Handle Rust-specific parsing errors

### With Documentation System
- Extract and format Rustdoc comments
- Preserve markdown formatting in doc strings
- Handle code examples in documentation

## Performance Considerations

- Optimize queries for common Rust patterns
- Efficient handling of large Rust files
- Minimal memory allocation during extraction
- Cache parsed query results

## Error Handling

- Graceful handling of macro-generated code
- Recovery from incomplete generic specifications
- Clear error messages for malformed Rust code
- Fallback extraction for unknown constructs

## Success Criteria

- Accurately extracts all major Rust language constructs
- Generates correct signatures with generics and lifetimes
- Properly extracts and formats Rustdoc comments
- Handles visibility modifiers and attributes correctly
- Performance suitable for large Rust codebases
- Comprehensive test coverage with real Rust examples
- Clean integration with the core parser framework

## Dependencies

- `tree-sitter-rust` parser
- Existing Tree-sitter infrastructure
- Core outline parser framework
- Standard library components

## Notes

Rust has complex syntax for generics, lifetimes, and trait bounds. The implementation should handle these accurately while remaining readable. Consider edge cases like higher-ranked trait bounds and associated type projections.