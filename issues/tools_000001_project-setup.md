# File Tools Project Setup

Refer to /Users/wballard/github/sah-filetools/ideas/tools.md

## Objective
Set up the foundational infrastructure for file editing tools in the MCP tools framework.

## Tasks
- [ ] Create `files/` module directory in `swissarmyhammer-tools/src/mcp/tools/`
- [ ] Set up basic module structure following established patterns
- [ ] Create `files/mod.rs` with module exports
- [ ] Add `files` module to parent `tools/mod.rs`
- [ ] Create placeholder subdirectories for each tool: `read/`, `edit/`, `write/`, `glob/`, `grep/`
- [ ] Implement basic registration function structure

## File Structure
```
swissarmyhammer-tools/src/mcp/tools/files/
├── mod.rs                  # Module exports and registration
├── shared_utils.rs         # Common file operation utilities
├── read/
│   ├── mod.rs
│   └── description.md
├── edit/
│   ├── mod.rs  
│   └── description.md
├── write/
│   ├── mod.rs
│   └── description.md
├── glob/
│   ├── mod.rs
│   └── description.md
└── grep/
    ├── mod.rs
    └── description.md
```

## Acceptance Criteria
- [ ] Module structure created following established patterns
- [ ] All directories and placeholder files created
- [ ] Module properly registered in parent mod.rs
- [ ] Project compiles successfully
- [ ] No breaking changes to existing functionality

## Notes
- Follow the same patterns used in `issues/`, `memoranda/`, etc.
- Ensure consistent naming conventions
- Set up for subsequent tool implementations

## Proposed Solution

After analyzing the existing codebase, I'll implement the file tools module following the established MCP tool registry pattern used in the `issues/`, `memoranda/`, and other modules.

### Implementation Steps

1. **Create the directory structure**: Set up the `files/` module directory following the pattern from existing modules
2. **Module exports**: Create `files/mod.rs` with proper module declarations and registration function
3. **Shared utilities**: Implement `shared_utils.rs` with common file operation utilities that all tools can use
4. **Tool placeholders**: Create structured placeholder directories for each tool (`read/`, `edit/`, `write/`, `glob/`, `grep/`)
5. **Tool stubs**: Implement basic `mod.rs` and `description.md` files for each tool following the established pattern
6. **Registration integration**: Add the files module to the parent `tools/mod.rs` and register it in `tool_registry.rs`
7. **Compilation verification**: Ensure the project compiles successfully with no breaking changes

### Pattern Analysis

From examining the codebase, I can see:
- Tools are organized in `swissarmyhammer-tools/src/mcp/tools/{category}/{tool}/`
- Each tool has a `mod.rs` implementing the `McpTool` trait 
- Each tool has a `description.md` with comprehensive documentation
- Category modules have a `register_{category}_tools()` function 
- The main `tools/mod.rs` declares all category modules
- The `tool_registry.rs` has registration functions that call category registration

This ensures consistency with the existing architecture and makes future tool implementations straightforward.

## Implementation Results

✅ **All tasks completed successfully!** 

### Structure Created

```
swissarmyhammer-tools/src/mcp/tools/files/
├── mod.rs                  # Module exports and registration function
├── shared_utils.rs         # Common file operation utilities with security validation
├── read/
│   ├── mod.rs             # ReadFileTool implementation stub
│   └── description.md     # Comprehensive tool documentation
├── edit/
│   ├── mod.rs             # EditFileTool implementation stub  
│   └── description.md     # Comprehensive tool documentation
├── write/
│   ├── mod.rs             # WriteFileTool implementation stub
│   └── description.md     # Comprehensive tool documentation
├── glob/
│   ├── mod.rs             # GlobFileTool implementation stub
│   └── description.md     # Comprehensive tool documentation
└── grep/
    ├── mod.rs             # GrepFileTool implementation stub
    └── description.md     # Comprehensive tool documentation
```

### Key Features Implemented

1. **Modular Structure**: Following established patterns from issues/, memoranda/, etc.
2. **Comprehensive Documentation**: Each tool has detailed description.md with examples and use cases  
3. **Shared Utilities**: Security validation, path checking, and error handling utilities
4. **Tool Registration**: Full integration with MCP tool registry system
5. **Compilation Success**: Project compiles without errors or breaking changes

### Tool Stubs Created

- `files_read`: File reading with support for various file types and partial reading
- `files_edit`: Precise string replacements with atomic operations  
- `files_write`: File creation and overwriting with directory management
- `files_glob`: Fast pattern matching with filtering and sorting
- `files_grep`: Content-based search using ripgrep integration

### Next Steps

The infrastructure is now ready for implementing the actual tool functionality. Each tool has:
- Complete MCP trait implementation structure
- JSON schema definitions for parameters
- Comprehensive documentation
- Integration with shared security utilities

All tools are registered and available through the MCP protocol, ready for implementation in subsequent issues.