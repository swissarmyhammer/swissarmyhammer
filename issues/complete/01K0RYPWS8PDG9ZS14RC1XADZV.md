sah issue needs a `next` command to show what will be next

and it needs to do this the exact same wap as mpc issue_next without duplicating logic

in fact all the cli issue commands need to work the same as the MCP, using the same code path, and just format the results nice

DO NOT reimplement logic in the CLI

## Proposed Solution

To avoid code duplication between CLI and MCP while ensuring they use identical logic:

1. **Add `get_next_issue` method to the `IssueStorage` trait** - This centralizes the logic for determining the next issue in the storage layer where it belongs
2. **Implement the method in all storage implementations** - `FileSystemIssueStorage`, `InstrumentedIssueStorage`, and `CachedIssueStorage`
3. **Update the MCP handler** - Replace the inline logic in `handle_issue_next` with a call to the new storage method
4. **Add `Next` variant to CLI `IssueCommands` enum** - Add the new command to the CLI command structure
5. **Implement CLI handler** - Create `show_next_issue` function that calls the same storage method
6. **Update CLI help text** - Document the new command for users

This approach ensures:
- ✅ **Single source of truth**: Logic lives in the storage layer
- ✅ **Zero code duplication**: CLI and MCP call the same method
- ✅ **Consistent behavior**: Identical results from CLI and MCP
- ✅ **Follows existing patterns**: Same approach used by other storage methods
- ✅ **Easy testing**: Logic only needs to be tested once in the storage layer

The implementation maintains the same algorithm as the original MCP handler:
1. Get all issues from storage (already sorted alphabetically)
2. Filter to pending issues only (`!issue.completed`)  
3. Return the first one (alphabetically first)
4. Return `None` if no pending issues exist

Both CLI and MCP now use this shared implementation while providing appropriate formatting for their respective interfaces.
