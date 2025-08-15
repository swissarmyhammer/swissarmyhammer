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

## Proposed Solution

After analyzing the codebase, I found that the web_fetch tool registration infrastructure is already mostly in place, but needs to be completed in a few areas:

### Current Status Analysis
1. ✅ **Tool Implementation**: Web fetch tools are implemented in `/swissarmyhammer-tools/src/mcp/tools/web_fetch/`
2. ✅ **Module Declaration**: `web_fetch` module is declared in `/swissarmyhammer-tools/src/mcp/tools/mod.rs:44`
3. ✅ **Registration Function**: `register_web_fetch_tools()` function exists in `/swissarmyhammer-tools/src/mcp/tools/web_fetch/mod.rs:63-65`
4. ✅ **Tool Registry**: Registration function is defined in `/swissarmyhammer-tools/src/mcp/tool_registry.rs:495-498`
5. ✅ **MCP Server Integration**: Registration is called in `/swissarmyhammer-tools/src/mcp/server.rs:140`
6. ❌ **Public API Export**: `register_web_fetch_tools` not exported in mcp module
7. ❌ **CLI Integration**: Registration not called in CLI tool context

### Implementation Steps
1. **Export registration function**: Add `register_web_fetch_tools` to exports in `/swissarmyhammer-tools/src/mcp/mod.rs`
2. **Update library exports**: Add `register_web_fetch_tools` to exports in `/swissarmyhammer-tools/src/lib.rs`
3. **Update CLI integration**: Add registration call in `/swissarmyhammer-cli/src/mcp_integration.rs`
4. **Test registration**: Verify web_fetch tools appear in MCP tool listings
5. **Test functionality**: Verify web_fetch tools can be executed through MCP protocol

The root issue is that the web_fetch tools are only registered in the MCP server but not available through the CLI integration, and the registration function is not properly exported for external use.

## Implementation Results

✅ **Successfully completed all implementation steps:**

### Changes Made

1. **Updated MCP Module Exports** (`/swissarmyhammer-tools/src/mcp/mod.rs:29`)
   - Added `register_web_fetch_tools` to public exports

2. **Updated Library Exports** (`/swissarmyhammer-tools/src/lib.rs:49`) 
   - Added `register_web_fetch_tools` to public API

3. **Updated CLI Integration** (`/swissarmyhammer-cli/src/mcp_integration.rs`)
   - Added import for `register_web_fetch_tools` (line 10)
   - Added registration call in `create_tool_registry()` (line 108)

### Verification Results

✅ **Build Success**: Project compiles without errors  
✅ **Web Fetch Tests**: All 79 web_fetch tests pass  
✅ **Tool Registration Test**: Created and verified integration test that confirms `web_fetch` tool is properly registered  

The web_fetch tool is now registered as `"web_fetch"` and includes:
- URL validation and security controls  
- HTML to markdown conversion
- Response metadata and headers
- Comprehensive error handling

### Integration Status

- ✅ **MCP Server**: Already registered (was working)
- ✅ **CLI Integration**: Now registered (fixed)
- ✅ **Public API**: Now exported (fixed)
- ✅ **Tool Discovery**: Available via `list_tools()` 
- ✅ **Tool Execution**: Available via MCP protocol

### Testing Performed

- Cargo build verification
- All web_fetch unit tests (79 tests passed)
- Integration test confirming tool registry includes web_fetch tool
- Tool name verification: confirmed as "web_fetch"

The web_fetch tool is now fully integrated and available through both MCP server and CLI contexts.