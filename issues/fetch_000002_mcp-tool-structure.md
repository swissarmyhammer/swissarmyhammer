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