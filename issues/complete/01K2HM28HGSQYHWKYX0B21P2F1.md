The logic for creating an abort file is duplicated. 

Do not do this -- when you need an abort file -- call the abort tool as a tool. Do not repeat the logic of the abort tool in other tools
## Proposed Solution

I've analyzed the codebase and found that the `GitOperations` struct in `swissarmyhammer/src/git.rs` has duplicated abort file creation logic. The issue is:

**Current Duplication:**
- `GitOperations::create_abort_file()` method manually creates `.swissarmyhammer/.abort` files (lines 720-738)
- This duplicates the functionality already provided by the MCP `abort_create` tool in `swissarmyhammer-tools/src/mcp/tools/abort/create/mod.rs`

**Solution Steps:**
1. **Remove duplicated method**: Delete the `create_abort_file()` method from `GitOperations` 
2. **Use MCP tool instead**: Replace all calls to `self.create_abort_file()` with proper MCP tool invocation
3. **Integrate with tool context**: Add necessary context to use the MCP abort tool from within git operations
4. **Update tests**: Modify tests to verify abort tool usage instead of direct file creation

**Benefits:**
- Eliminates code duplication
- Centralizes abort file creation logic in the MCP tool
- Ensures consistent abort file handling throughout the system
- Follows the established pattern of delegating to MCP tools

The changes will be made to:
- `swissarmyhammer/src/git.rs` - Remove `create_abort_file()` method and update callers
- Test files - Update to work with MCP tool integration

This refactoring follows the architectural principle of using MCP tools as the single source of truth for file operations.