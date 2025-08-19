issue_mark_complete needs to support the special name  which will mark the current issue complete.
issue_mark_complete needs to support the special name  which will mark the current issue complete.

## Proposed Solution

Based on examining the `issue_show` tool implementation, I will update the `issue_mark_complete` tool to support the special name "current" by:

1. **Adding logic to handle the "current" special name**: When the name parameter is "current", the tool will:
   - Get the current git branch using the git operations context
   - Strip the issue branch prefix (e.g., "issue/") from the branch name to get the issue name
   - Use that resolved issue name to mark the issue complete

2. **Following the existing pattern**: The implementation will mirror how `issue_show` handles the "current" special name, using the same git operations and config pattern.

3. **Maintaining backward compatibility**: Regular issue names will continue to work as before.

The key changes will be in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/src/mcp/tools/issues/mark_complete/mod.rs` to add the special name resolution logic before calling the storage layer.

## Implementation Notes

Successfully implemented the "current" special name support for the `issue_mark_complete` tool:

### Changes Made

1. **Updated imports**: Added `use swissarmyhammer::config::Config;` to access global configuration for issue branch prefix.

2. **Enhanced schema**: Updated the schema description to mention the "current" special name support: "Issue name to mark as complete. Use 'current' to mark the current issue complete."

3. **Added special name handling logic**: The execute method now checks if the name parameter is "current" and:
   - Gets the current git branch using the git operations context
   - Strips the issue branch prefix from the branch name to get the actual issue name
   - Returns appropriate errors if not on an issue branch or if git operations are unavailable
   - Falls back to using the original name parameter for regular issue names

4. **Type handling**: Fixed the type mismatch by accessing the underlying String value using `.0` from the `IssueName` wrapper type.

### Testing

- All 483 tests pass without failures
- No clippy linting warnings
- Code formatted with `cargo fmt`

The implementation follows the exact same pattern used in the `issue_show` tool for handling the "current" special name, ensuring consistency across the issue tool suite.

### File Changed

- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/src/mcp/tools/issues/mark_complete/mod.rs`