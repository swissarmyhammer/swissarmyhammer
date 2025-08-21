# OUTLINE_000243: Project Setup and Tool Structure

Refer to ./specification/outline_tool.md

## Summary

Create the foundational MCP tool structure for the outline tool following established patterns in the SwissArmyHammer codebase. This includes setting up the directory structure, basic types, and tool registration framework.

## Context

The outline tool will generate structured code overviews using Tree-sitter parsing. It follows the established MCP tool pattern of `src/mcp/tools/<noun>/<verb>/` organization that's used throughout the codebase.

## Requirements

### 1. Create Tool Directory Structure
```
src/mcp/tools/outline/
├── generate/
│   ├── mod.rs
│   └── description.md
└── mod.rs
```

### 2. Basic Type Definitions
Create core data structures in `src/mcp/tools/outline/generate/mod.rs`:
- `OutlineRequest` - Input parameters (patterns, output_format)
- `OutlineResponse` - Tool response structure
- `OutlineNode` - Individual symbol representation
- `OutlineKind` - Enum for symbol types (class, function, method, etc.)

### 3. Tool Description
Create `description.md` with comprehensive documentation following existing pattern:
- Tool purpose and capabilities
- Parameter descriptions with examples
- Usage examples for different scenarios
- Supported languages and features

### 4. MCP Tool Registration
- Add tool to `src/mcp/tools/mod.rs`
- Register in tool handler registry
- Follow established registration patterns

## Technical Details

### OutlineRequest Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineRequest {
    pub patterns: Vec<String>,           // Glob patterns
    pub output_format: Option<String>,   // Default "yaml"
}
```

### OutlineNode Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    pub name: String,
    pub kind: OutlineKind,
    pub line: u32,
    pub signature: Option<String>,
    pub doc: Option<String>,
    pub type_info: Option<String>,
    pub children: Option<Vec<OutlineNode>>,
}
```

### OutlineKind Enum
Support all major code constructs:
- Class, Interface, Struct, Enum
- Function, Method, Constructor
- Property, Field, Variable
- Module, Namespace
- Type alias, Trait

## Implementation Steps

1. Create directory structure under `src/mcp/tools/outline/`
2. Implement basic data structures in `generate/mod.rs`
3. Create comprehensive tool description in `description.md`
4. Add tool registration to module system
5. Create initial unit tests for type definitions
6. Verify tool appears in MCP tool registry

## Testing Requirements

- Unit tests for all data structures
- Serialization/deserialization tests
- Tool registration verification
- Description markdown validation

## Dependencies

- `serde` for serialization
- `serde_yaml` for YAML output
- Existing MCP tool infrastructure
- Standard library components

## Success Criteria

- Tool directory structure matches established patterns
- All types serialize/deserialize correctly
- Tool description is comprehensive and clear
- Tool appears in MCP server tool list
- Unit tests pass with good coverage
- Code follows established style and patterns

## Notes

This step establishes the foundation for all subsequent outline tool development. The structure should be extensible to support additional output formats and language features in future iterations.
## Proposed Solution

I will implement the foundational MCP tool structure for the outline tool following these steps:

1. **Create Tool Directory Structure** ✅
   - Created `src/mcp/tools/outline/generate/` with proper module organization
   - Follows established `<noun>/<verb>/` pattern used throughout codebase

2. **Implement Core Data Structures** ✅
   - `OutlineRequest`: Input parameters with patterns and output_format
   - `OutlineResponse`: Tool response with outline data and metadata
   - `OutlineNode`: Individual symbol representation with hierarchical children
   - `OutlineKind`: Comprehensive enum for all symbol types (class, function, method, etc.)

3. **Create MCP Tool Implementation** ✅
   - `OutlineGenerateTool` implementing `McpTool` trait
   - Comprehensive JSON schema with validation
   - Proper argument parsing and error handling
   - Support for YAML and JSON output formats

4. **Add Tool Description** ✅
   - Comprehensive `description.md` with usage examples
   - Documentation of supported languages and symbol types
   - Clear parameter descriptions and output structure
   - Performance characteristics and use cases

5. **Register Tool in Module System** ✅
   - Added `outline` module to `src/mcp/tools/mod.rs`
   - Created `register_outline_tools()` function in `src/mcp/tool_registry.rs`
   - Registered tool in `src/mcp/server.rs` alongside existing tools

6. **Create Comprehensive Unit Tests** ✅
   - Serialization/deserialization tests for all data structures
   - Schema validation tests
   - Tool execution tests with valid and invalid inputs
   - Error handling validation

## Implementation Status: COMPLETE

The foundational structure is now fully implemented and all tests are passing. The tool is properly registered in the MCP server and ready for the next development phase (Tree-sitter integration).

### Key Features Implemented:
- Full type safety with Rust's strong typing
- Comprehensive error handling and validation
- Support for multiple output formats (YAML/JSON)
- Extensible design for future language support
- Complete test coverage including edge cases
- Proper documentation following project standards

### Next Steps:
This foundation enables the subsequent outline tool issues to focus on:
- Tree-sitter parser integration
- Language-specific symbol extraction
- File discovery and processing
- Output formatting enhancements