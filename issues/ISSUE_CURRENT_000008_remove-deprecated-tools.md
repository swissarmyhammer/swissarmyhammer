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