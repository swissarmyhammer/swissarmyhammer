# Enhance issue_show Tool with "next" Parameter  

Refer to ./specification/issue_current.md

## Goal

Add support for `"next"` as a special parameter value in the `issue_show` tool to replace `issue_next` functionality.

## Tasks

1. **Implement next issue detection logic**:
   - Add method to detect when `name == "next"`
   - Integrate next issue selection logic from `NextIssueTool`
   - Use same storage backend access pattern
   - Handle case when no pending issues exist

2. **Add storage backend integration**:
   - Access `context.issue_storage` in the execute method
   - Use `get_next_issue()` method from storage
   - Handle async operations properly
   - Maintain same error handling patterns

3. **Preserve response formatting**:
   - Return issue details for next pending issue (not just the name)
   - Use same formatting as regular `issue_show` for consistency
   - Handle "no pending issues" case with appropriate message
   - Maintain same error handling patterns

4. **Ensure consistency with current tool**:
   - Same alphabetical ordering logic
   - Same pending issue detection
   - Same response format as existing `NextIssueTool`
   - Handle all edge cases appropriately

5. **Update special parameter handling**:
   - Handle both "current" and "next" special parameters
   - Ensure they work independently and consistently
   - Add proper validation for special parameter values

## Expected Outcome

`issue_show next` works identically to `issue_next` tool:
- Returns next pending issue details in same format as regular issue_show
- Returns informative message when no pending issues exist
- Handles all error cases appropriately
- Works alongside "current" parameter functionality

## Success Criteria

- `issue_show next` returns next pending issue with full details
- Same selection logic as original `issue_next` tool
- Consistent formatting with regular `issue_show` output
- Both "current" and "next" parameters work correctly together
- All error cases handled consistently