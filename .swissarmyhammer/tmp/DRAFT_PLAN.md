# Draft Plan: Outline Tool Implementation

## Overview
Implementation of the outline tool as specified in `./specification/outline_tool.md`. This tool provides structured code overviews using Tree-sitter parsing across multiple programming languages (Rust, TypeScript, JavaScript, Dart, Python).

## Current State Analysis
- MCP tools follow established pattern in `src/mcp/tools/` with noun/verb structure
- Existing search tools use Tree-sitter and are in `src/mcp/tools/search/`
- Tree-sitter parsing infrastructure already exists in `src/search/parser.rs`
- DuckDB storage infrastructure exists in `src/search/storage.rs`
- Multi-language parsing capabilities already implemented

## Requirements from Specification
1. Tree-sitter parsing for multiple languages (Rust, TS, JS, Dart, Python)
2. Hierarchical YAML output structure mirroring file system
3. Extracts: types, functions, methods, properties, docs, signatures, line numbers
4. Glob pattern input with gitignore respect
5. Error handling for parsing failures
6. Language-specific handling for constructs
7. MCP tool integration following existing patterns

## Architecture Analysis
The outline tool should:
- Leverage existing Tree-sitter infrastructure from search module
- Create new MCP tool under `src/mcp/tools/outline/` following established patterns
- Reuse gitignore handling and file discovery logic
- Generate YAML output instead of search indexing
- Extract detailed symbol information including signatures and documentation

## Implementation Strategy

### Phase 1: Core Infrastructure Setup
1. Create MCP tool structure following established patterns
2. Set up basic outline data structures and types
3. Implement file discovery with gitignore respect
4. Create YAML output formatting infrastructure

### Phase 2: Tree-sitter Integration
5. Extend existing Tree-sitter parser for outline extraction
6. Implement language-specific symbol extraction
7. Create hierarchical symbol tree building
8. Add signature and documentation extraction

### Phase 3: Output Generation
9. Implement file system hierarchy mirroring in YAML
10. Create detailed symbol information formatting
11. Add error handling and reporting
12. Implement comprehensive testing

### Phase 4: Integration and Polish
13. Register tool with MCP server
14. Add CLI integration if needed
15. Create comprehensive documentation
16. Performance optimization and cleanup

## Implementation Breakdown

### Small Incremental Steps

1. **Project Setup & Tool Structure** - Create MCP tool directory structure, basic types
2. **File Discovery** - Implement glob pattern processing with gitignore integration
3. **Tree-sitter Core Integration** - Extend parser for outline-specific extraction
4. **Rust Language Support** - Implement Rust-specific symbol extraction
5. **TypeScript/JavaScript Support** - Implement TS/JS symbol extraction
6. **Dart Language Support** - Implement Dart symbol extraction  
7. **Python Language Support** - Implement Python symbol extraction
8. **Hierarchical Structure Builder** - Build nested symbol trees
9. **YAML Output Formatter** - Generate structured YAML output
10. **Signature Extraction** - Extract function/method signatures
11. **Documentation Extraction** - Extract and format documentation comments
12. **Error Handling** - Robust error handling for parsing failures
13. **Testing Infrastructure** - Comprehensive unit and integration tests
14. **MCP Registration** - Register tool with MCP server
15. **Performance Optimization** - Optimize for large codebases

Each step should be small enough to implement safely (< 500 lines of code) and build incrementally on previous steps.

## Dependencies
- Existing Tree-sitter infrastructure in `src/search/`
- MCP tool patterns from `src/mcp/tools/`
- YAML serialization capabilities (serde_yaml)
- File system utilities from existing codebase
- Gitignore processing from existing search tools

## Success Criteria
- Tool generates accurate YAML outlines for all supported languages
- Respects gitignore patterns correctly
- Provides rich symbol information (signatures, docs, locations)
- Follows established MCP tool patterns
- Comprehensive test coverage
- Performance suitable for large codebases