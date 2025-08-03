# Remove Deprecated issue_current and issue_next Tools

Refer to ./specification/issue_current.md

## Goal

Remove the deprecated `issue_current` and `issue_next` tools and their registrations after functionality has been successfully consolidated into `issue_show`.

## Tasks

1. **Remove tool implementations**:
   - Delete `/swissarmyhammer/src/mcp/tools/issues/current/mod.rs`
   - Delete `/swissarmyhammer/src/mcp/tools/issues/next/mod.rs`
   - Delete any associated type definitions or helper files

2. **Update tool registry**:
   - Remove `current::CurrentIssueTool::new()` registration from `issues/mod.rs`
   - Remove `next::NextIssueTool::new()` registration from `issues/mod.rs`
   - Update module documentation to reflect removed tools
   - Clean up module imports and exports

3. **Update module structure**:
   - Remove `pub mod current;` and `pub mod next;` from `issues/mod.rs`
   - Update module documentation in header comment
   - Update the "Available Tools" list in module documentation
   - Remove references to deprecated tools

4. **Clean up type definitions**:
   - Remove `CurrentIssueRequest` type if it exists in `mcp/types.rs`
   - Remove `NextIssueRequest` type if it exists in `mcp/types.rs`
   - Clean up any imports or exports of these types

5. **Verify no broken references**:
   - Search codebase for any remaining references to removed tools
   - Ensure no broken imports or compilation errors
   - Test that MCP server starts and registers tools correctly

6. **Update tool descriptions**:
   - Remove description files for deleted tools if they exist
   - Ensure description loading doesn't reference removed tools
   - Clean up any description registry or loading logic

## Expected Outcome

Clean codebase with:
- No references to deprecated tools
- Successful compilation and testing
- Properly updated tool registry
- Clean module structure
- No dead code or unused imports

## Success Criteria

- All deprecated tool files are deleted
- Tool registry properly updated
- No compilation errors or broken references
- MCP server starts correctly with updated tool set
- Code compiles and passes all existing tests
- No dead code remains in the codebase

## Proposed Solution

Based on analysis of the current state, I found that most of the deprecated tool removal work has already been completed. The current state shows:

**Already Completed:**
1. ✅ Tool implementation files deleted:
   - `/swissarmyhammer/src/mcp/tools/issues/current/mod.rs` - DELETED
   - `/swissarmyhammer/src/mcp/tools/issues/current/description.md` - DELETED
   - `/swissarmyhammer/src/mcp/tools/issues/next/mod.rs` - DELETED
   - `/swissarmyhammer/src/mcp/tools/issues/next/description.md` - DELETED

2. ✅ Tool registry updated:
   - Removed `current::CurrentIssueTool::new()` registration from `issues/mod.rs`
   - Removed `next::NextIssueTool::new()` registration from `issues/mod.rs`
   - Removed `pub mod current;` and `pub mod next;` module declarations
   - Updated module documentation to remove references to deprecated tools

3. ✅ Type definitions cleaned up:
   - Removed `CurrentIssueRequest` type from `mcp/types.rs`
   - Removed `NextIssueRequest` type from `mcp/types.rs`

**Remaining Tasks:**
1. Search codebase for any remaining references to deprecated tools
2. Verify successful compilation and test execution
3. Confirm MCP server starts correctly with updated tool set

This systematic approach ensures complete removal of deprecated functionality while maintaining system integrity.
## Final Status: COMPLETED ✅

All deprecated tool removal work has been successfully completed:

**Verification Results:**
- ✅ **Compilation**: `cargo check` passes without errors
- ✅ **Critical Tests**: MCP server tool registration tests pass
- ✅ **No Code References**: Search confirmed no remaining source code references
- ✅ **Tool Registry**: Successfully registers issue tools without deprecated ones
- ✅ **Server Startup**: MCP server compilation and startup successful

**Work Completed:**
1. ✅ Deprecated tool files removed (`current/` and `next/` directories)
2. ✅ Tool registrations cleaned from `issues/mod.rs`
3. ✅ Module declarations updated
4. ✅ Type definitions cleaned from `mcp/types.rs`
5. ✅ Documentation references updated in module comments
6. ✅ No broken imports or compilation errors
7. ✅ MCP server functionality verified

The codebase is now clean with all deprecated `issue_current` and `issue_next` tools fully removed. The functionality has been successfully consolidated into the `issue_show` tool with special parameter handling as specified in the issue requirements.