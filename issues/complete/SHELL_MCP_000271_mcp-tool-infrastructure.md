# MCP Shell Tool Infrastructure Setup

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Set up the foundational MCP tool infrastructure for the shell command execution tool following the established patterns in the SwissArmyHammer codebase.

## Objective

Create the basic directory structure, tool registration, and description files for the new shell MCP tool, following the noun/verb organization pattern used by existing tools.

## Requirements

### Directory Structure
- Create `swissarmyhammer-tools/src/mcp/tools/shell/` directory
- Create `swissarmyhammer-tools/src/mcp/tools/shell/execute/` subdirectory  
- Follow the established noun/verb pattern used by other tools (issues/, memoranda/, etc.)

### Tool Registration Files
- Create `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs` for module exports
- Create `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` for tool implementation
- Create `swissarmyhammer-tools/src/mcp/tools/shell/execute/description.md` for tool description

### Tool Description
The `description.md` file should include:
- Tool purpose and functionality overview
- Parameter specifications matching the JSON schema from the specification
- Response format documentation
- Usage examples for common scenarios
- Security considerations and limitations

### Module Integration
- Update parent `mod.rs` files to include the new shell tool module
- Ensure proper exports and module visibility
- Follow existing patterns from issues/, memoranda/, and other tool modules

## Implementation Details

### Parameters Schema
Implement the complete parameter schema from specification:
```json
{
  "command": {"type": "string", "description": "Shell command to execute"},
  "working_directory": {"type": "string", "optional": true},
  "timeout": {"type": "integer", "default": 300, "min": 1, "max": 1800},
  "environment": {"type": "object", "optional": true}
}
```

### Response Format
Document the response format including:
- Successful execution metadata
- Command failure handling
- Timeout error responses
- Structured output with execution details

## Acceptance Criteria

- [ ] Directory structure created following noun/verb pattern
- [ ] Module files created with proper exports
- [ ] Tool description file contains comprehensive documentation
- [ ] Module integration completed without breaking existing tools
- [ ] Tool appears in MCP tool registry when server starts
- [ ] Tool description accessible via MCP protocol

## Notes

- This step sets up the foundation for all subsequent shell tool implementation
- Follow the exact patterns used by existing tools for consistency
- Ensure the tool integrates cleanly with the existing MCP server architecture
- The description file will serve as the primary documentation for users

## Proposed Solution

Based on my analysis of the existing SwissArmyHammer codebase, I will implement the MCP shell tool infrastructure following the established patterns:

### 1. Directory Structure
- Create `swissarmyhammer-tools/src/mcp/tools/shell/` as the main directory
- Create `swissarmyhammer-tools/src/mcp/tools/shell/execute/` as the noun/verb subdirectory
- Follow the exact pattern used by existing tools (issues/, memoranda/, etc.)

### 2. Module Files
- `shell/mod.rs`: Module organization with documentation and registration function
- `shell/execute/mod.rs`: Tool implementation following McpTool trait
- `shell/execute/description.md`: Comprehensive tool documentation

### 3. Integration Points
- Update `tools/mod.rs` to include the shell module
- Add registration function call in `tool_registry.rs` 
- Add registration call in `server.rs` where other tools are registered
- Update exports in appropriate `mod.rs` files

### 4. Tool Implementation
- Follow existing patterns from issues/create/mod.rs
- Implement McpTool trait with proper error handling
- Use BaseToolImpl for argument parsing and response creation
- Include comprehensive JSON schema for validation
- Follow the same naming convention: `shell_execute`

### 5. Tool Description
- Create comprehensive Markdown documentation
- Include parameter specifications matching the JSON schema
- Document response formats for success, failure, and timeout cases
- Provide usage examples and security considerations
- Reference the ideas/shell.md specification for detailed requirements

This approach ensures complete consistency with the existing codebase architecture while setting up the foundation for all subsequent shell tool development.
## Implementation Complete ✅

The MCP shell tool infrastructure has been successfully implemented following all established patterns and meeting all acceptance criteria.

### What Was Implemented

1. **✅ Directory Structure Created**
   - `swissarmyhammer-tools/src/mcp/tools/shell/` - Main shell module directory
   - `swissarmyhammer-tools/src/mcp/tools/shell/execute/` - Execute tool implementation
   - Follows the exact noun/verb pattern used by existing tools

2. **✅ Tool Implementation**
   - `shell/execute/mod.rs` - Complete McpTool trait implementation with comprehensive tests
   - Implements full parameter validation and error handling
   - Follows all established patterns from existing tools
   - Returns structured placeholder response for infrastructure testing

3. **✅ Tool Description**
   - `shell/execute/description.md` - Comprehensive tool documentation
   - Covers all parameters, response formats, usage examples
   - Documents security considerations and error handling
   - Matches the format and quality of existing tool descriptions

4. **✅ Module Organization**
   - `shell/mod.rs` - Complete module organization with registration function
   - Includes comprehensive documentation and test coverage
   - Follows exact patterns from other tool modules (issues, memoranda, etc.)

5. **✅ Tool Registration**
   - Added `register_shell_tools()` function to `tool_registry.rs`
   - Updated `server.rs` to call shell tool registration
   - Updated all module exports (`mod.rs`, `lib.rs`)
   - Shell tools are now automatically included in MCP server

6. **✅ Comprehensive Testing**
   - 12 passing tests covering all aspects of tool registration and validation
   - Parameter validation tests (required/optional fields, ranges, empty values)
   - Tool registry integration tests
   - MCP server capability exposure tests
   - No breaking changes to existing functionality

### Verification Results

- **✅ Compilation**: Clean build with only expected warning about unused `environment` field
- **✅ Tests**: All 12 shell-related tests pass
- **✅ Integration**: Shell tools properly registered and exposed via MCP server
- **✅ Tool Schema**: Complete JSON schema with proper validation
- **✅ Documentation**: Tool description accessible via MCP protocol

### Key Implementation Details

- **Tool Name**: `shell_execute` (follows established naming convention)
- **Parameters**: Complete implementation of `command`, `working_directory`, `timeout`, `environment`
- **Validation**: Comprehensive parameter validation with appropriate error messages
- **Response**: Structured placeholder response indicating infrastructure readiness
- **Testing**: Full test coverage including edge cases and error conditions

### Next Steps

This infrastructure setup provides the foundation for subsequent issues:
- Issue 272: Core shell execution engine implementation 
- Issue 273: Timeout and process management
- Issue 274: Working directory and environment handling
- And so on...

The tool is now fully registered and discoverable via MCP, ready for the actual command execution implementation in the next phase.