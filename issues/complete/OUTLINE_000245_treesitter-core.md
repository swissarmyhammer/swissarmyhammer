# OUTLINE_000245: Tree-sitter Core Integration

Refer to ./specification/outline_tool.md

## Summary

Extend the existing Tree-sitter parsing infrastructure to support outline extraction, building on the parser foundations already established in the search module but with different extraction requirements.

## Context

The existing search module has Tree-sitter parsing capabilities for indexing purposes. The outline tool needs similar parsing but with focus on extracting structured symbol information rather than creating search indexes.

## Requirements

### 1. Outline Parser Module
Create `src/outline/parser.rs` that extends existing parser capabilities:
- Parse files into abstract syntax trees
- Extract symbol definitions with hierarchical relationships
- Capture source locations, signatures, and documentation
- Handle parsing errors gracefully

### 2. Core Parser Infrastructure
```rust
pub struct OutlineParser {
    language: Language,
    parser: tree_sitter::Parser,
}

impl OutlineParser {
    pub fn new(language: Language) -> Result<Self>;
    pub fn parse_file(&mut self, content: &str) -> Result<OutlineTree>;
    pub fn parse_source(&mut self, source: &str, file_path: &Path) -> Result<OutlineTree>;
}

pub struct OutlineTree {
    pub root: OutlineNode,
    pub file_path: PathBuf,
    pub language: Language,
}
```

### 3. Symbol Extraction Framework
- Generic symbol extraction that works across languages
- Language-specific extraction rules
- Hierarchical symbol relationships (classes contain methods, etc.)
- Source location tracking for all symbols

### 4. Tree-sitter Query Integration
- Use Tree-sitter queries for consistent symbol extraction
- Define language-specific queries for common patterns
- Capture groups for names, types, documentation
- Error handling for malformed source code

## Technical Details

### Core Extraction Logic
```rust
pub trait SymbolExtractor {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<OutlineNode>;
    fn extract_documentation(&self, node: &Node, source: &str) -> Option<String>;
    fn extract_signature(&self, node: &Node, source: &str) -> Option<String>;
}

pub struct GenericExtractor {
    queries: HashMap<Language, Query>,
}
```

### Tree-sitter Queries
Define queries for each language to extract:
- Function/method definitions
- Class/struct/enum definitions  
- Module/namespace definitions
- Property/field definitions
- Documentation comments
- Type information

Example query structure:
```scheme
(function_item
  name: (identifier) @name
  parameters: (parameters) @params
  body: (block) @body
  (#match? @name "^[a-zA-Z_]"))
```

### Error Recovery
- Continue parsing when individual symbols fail
- Report parsing errors with context
- Skip malformed sections gracefully
- Provide partial results when possible

## Implementation Steps

1. Create `src/outline/parser.rs` module
2. Implement basic Tree-sitter parser wrapper
3. Create symbol extraction trait and generic implementation
4. Define Tree-sitter queries for common patterns
5. Add error handling and recovery mechanisms
6. Implement source location tracking
7. Create comprehensive unit tests

## Integration with Existing Code

### Reuse from Search Module
- Tree-sitter language loading patterns
- Parser initialization and error handling
- File content reading utilities
- Language detection logic

### Differences from Search Parsing
- Focus on symbol extraction vs. chunking for search
- Preserve hierarchical relationships
- Extract more detailed metadata (signatures, docs)
- Different error handling requirements

## Testing Requirements

### Unit Tests
- Parser initialization for all languages
- Symbol extraction accuracy
- Error handling for malformed code
- Source location accuracy
- Documentation extraction

### Integration Tests
- Parse real source files from the project
- Verify hierarchical relationships
- Test error recovery mechanisms
- Performance with large files

### Test Data
- Create test files for each supported language
- Include edge cases (nested classes, complex signatures)
- Test malformed code scenarios
- Document expected behavior

## Performance Considerations

- Reuse parser instances where possible
- Efficient memory usage for large files
- Parallel parsing for multiple files
- Query optimization for common patterns

## Error Handling Strategy

- Graceful degradation for parsing failures
- Clear error messages with source locations
- Recovery mechanisms for partial parsing
- Logging for debugging parser issues

## Success Criteria

- Successfully parses files in all supported languages
- Extracts accurate symbol hierarchies
- Handles parsing errors gracefully
- Provides detailed source location information
- Performance suitable for large files
- Comprehensive test coverage
- Clean integration with existing Tree-sitter infrastructure

## Dependencies

- Existing `tree-sitter` crate and language bindings
- Tree-sitter language parsers for Rust, TypeScript, JavaScript, Dart, Python
- Query parsing capabilities
- Standard library components

## Notes

This step establishes the core parsing engine for the outline tool. The parser should be robust and extensible to support additional languages in the future. Consider performance implications as this will be used on potentially large codebases.

## Proposed Solution

After analyzing the existing codebase, I'll extend the search module's Tree-sitter parser infrastructure to support outline extraction. The key differences from search parsing are:

### 1. Outline-Specific Data Structures
```rust
// Core outline structures building on search Language enum
pub struct OutlineNode {
    pub name: String,
    pub node_type: OutlineNodeType,
    pub start_line: usize,
    pub end_line: usize,
    pub children: Vec<Box<OutlineNode>>,
    pub signature: Option<String>,
    pub documentation: Option<String>,
    pub visibility: Option<Visibility>,
}

pub enum OutlineNodeType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Module,
    Property,
    Constant,
    Variable,
}

pub struct OutlineTree {
    pub root: OutlineNode,
    pub file_path: PathBuf,
    pub language: Language,
    pub symbols: Vec<OutlineNode>,
}
```

### 2. Parser Architecture
```rust
pub struct OutlineParser {
    code_parser: CodeParser,  // Reuse existing search parser
    extractors: HashMap<Language, Box<dyn SymbolExtractor>>,
}

pub trait SymbolExtractor: Send + Sync {
    fn extract_symbols(&self, tree: &Tree, source: &str) -> Vec<OutlineNode>;
    fn extract_documentation(&self, node: &Node, source: &str) -> Option<String>;
    fn extract_signature(&self, node: &Node, source: &str) -> Option<String>;
    fn extract_hierarchical_structure(&self, nodes: Vec<OutlineNode>) -> Vec<OutlineNode>;
}
```

### 3. Enhanced Tree-sitter Queries
Unlike search queries that focus on chunking, outline queries will capture:
- Detailed symbol metadata (names, types, visibility)
- Hierarchical relationships (classes contain methods)
- Documentation comments
- Function signatures with parameters and return types

### 4. Implementation Steps
1. **Create outline-specific types** extending search Language enum
2. **Build OutlineParser wrapper** around existing CodeParser
3. **Implement SymbolExtractor trait** with language-specific implementations  
4. **Define comprehensive Tree-sitter queries** for symbol extraction
5. **Add hierarchical relationship building** to organize symbols
6. **Implement comprehensive error handling** with graceful degradation
7. **Create extensive unit and integration tests**

### 5. Integration Points
- **Reuse search/parser.rs** infrastructure for Tree-sitter setup
- **Extend search/types.rs** Language enum for consistency
- **Build on existing** query patterns and error handling
- **Share Tree-sitter language** configurations and parsers

This approach maximizes code reuse while providing outline-specific functionality focused on symbol extraction rather than search indexing.