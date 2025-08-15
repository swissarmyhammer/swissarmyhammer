# Create Web Fetch MCP Tool Structure

## Overview
Create the basic directory structure and scaffolding for the web_fetch MCP tool following the established MCP tool directory pattern. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/` directory
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/` subdirectory
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/mod.rs` with exports
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs` with basic struct
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/description.md` with tool description
- Update `swissarmyhammer-tools/src/mcp/tools/mod.rs` to include web_fetch module

## Implementation Details
- Follow the noun/verb pattern: `web_fetch/fetch/`
- Create `WebFetchTool` struct implementing `McpTool` trait
- Define basic JSON schema for parameters (url, timeout, etc.)
- Use `include_str!` for description loading
- Add registration function `register_web_fetch_tools()`

## Success Criteria
- Directory structure matches existing tools (memoranda, issues, etc.)
- Basic tool struct compiles successfully
- Tool appears in registry when registered
- Description file follows documentation standards

## Dependencies
- Requires fetch_000001_markdowndown-dependency (for dependency availability)

## Estimated Impact
- Creates foundation for web fetch tool implementation
- Enables iterative development of functionality

## Proposed Solution

Based on my analysis of the existing MCP tool structure and the fetch.md specification, I will implement the web_fetch tool following the established patterns:

### 1. Directory Structure
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/` (noun-based organization)
- Create `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/` (verb-based submodule)
- Each tool module includes `mod.rs` (implementation) and `description.md` (documentation)

### 2. Implementation Pattern
- `WebFetchTool` struct implementing `McpTool` trait with:
  - `name()` returning "web_fetch" 
  - `description()` loading from `description.md` using `include_str!`
  - `schema()` defining JSON schema for parameters (url, timeout, follow_redirects, etc.)
  - `execute()` async implementation (will be placeholder for now)

### 3. Integration Points  
- Registration function `register_web_fetch_tools(registry)` in web_fetch/mod.rs
- Update main tools/mod.rs to include web_fetch module
- Follow existing patterns from issues, memoranda, etc.

### 4. Tool Schema
Based on fetch.md specification:
- Required: url (string with URI format)
- Optional: timeout (integer, 5-120s, default 30)  
- Optional: follow_redirects (boolean, default true)
- Optional: max_content_length (integer, 1KB-10MB, default 1MB)
- Optional: user_agent (string, default "SwissArmyHammer-Bot/1.0")

This creates the foundation for iterative development while maintaining consistency with the existing codebase architecture.

## Implementation Completed

Successfully created the MCP tool structure for web_fetch following all established patterns:

### ✅ Directory Structure Created
- `swissarmyhammer-tools/src/mcp/tools/web_fetch/` (noun-based organization)  
- `swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/` (verb-based submodule)
- Follows exact same pattern as issues/, memoranda/, etc.

### ✅ Core Files Implemented
- **web_fetch/mod.rs**: Module exports and registration function `register_web_fetch_tools()`
- **web_fetch/fetch/mod.rs**: `WebFetchTool` struct implementing `McpTool` trait
- **web_fetch/fetch/description.md**: Comprehensive tool documentation
- **types.rs**: Added `WebFetchRequest` struct with proper validation

### ✅ Schema Implementation
Implemented full JSON schema based on fetch.md specification:
- Required: `url` (string with URI format)
- Optional: `timeout` (5-120s, default 30)
- Optional: `follow_redirects` (boolean, default true)  
- Optional: `max_content_length` (1KB-10MB, default 1MB)
- Optional: `user_agent` (string, default "SwissArmyHammer-Bot/1.0")

### ✅ Registration Integration  
- Added `register_web_fetch_tools()` to `tool_registry.rs`
- Updated `server.rs` to call registration function
- Added imports to main `tools/mod.rs`
- Follows exact same pattern as all existing tools

### ✅ Validation & Security
- URL scheme validation (HTTP/HTTPS only)
- Parameter range validation (timeout, content length)  
- Rate limiting integration
- Input validation using shared utilities

### ✅ Compilation Success
- `cargo check --package swissarmyhammer-tools` ✅  
- `cargo test --package swissarmyhammer-tools web_fetch --no-run` ✅
- All imports resolve correctly
- Tool properly registered in MCP server

### 📝 Implementation Notes
- Used placeholder execute() method that validates parameters and returns success message
- Actual web fetching with markdowndown crate will be implemented in subsequent issues
- Follows TDD approach - structure first, then functionality
- All error handling patterns match existing tools

### ✅ Success Criteria Met
- ✅ Directory structure matches existing tools (memoranda, issues, etc.)
- ✅ Basic tool struct compiles successfully  
- ✅ Tool appears in registry when registered
- ✅ Description file follows documentation standards
- ✅ Dependency on fetch_000001_markdowndown-dependency satisfied

The web_fetch tool structure is now ready for iterative development of the actual fetching functionality.