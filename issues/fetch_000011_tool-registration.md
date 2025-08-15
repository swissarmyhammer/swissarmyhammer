# Register Web Fetch Tool in MCP Registry

## Overview
Register the web_fetch tool with the MCP tool registry so it becomes available for use through the MCP protocol. Refer to /Users/wballard/github/sah-fetch/ideas/fetch.md.

## Tasks
- Add web_fetch tool registration to the main tool registry
- Update tool registry to include web_fetch tools
- Verify tool appears in MCP tool listings
- Test tool registration and availability through MCP protocol
- Update any CLI integrations if needed

## Implementation Details
- Add `register_web_fetch_tools()` call to main registry initialization
- Update `swissarmyhammer-tools/src/mcp/tools/mod.rs` exports
- Verify tool registry includes web_fetch properly
- Test that tool is discoverable through MCP list_tools
- Ensure tool context is properly passed to execution

## Success Criteria
- web_fetch tool appears in MCP tool listings
- Tool can be executed through MCP protocol
- Tool registration follows existing patterns
- Integration with existing MCP server works correctly
- Tool context (storage, services) is available during execution

## Dependencies
- Requires fetch_000010_integration-tests (for complete implementation)

## Estimated Impact
- Makes web_fetch tool available for use
- Completes MCP integration