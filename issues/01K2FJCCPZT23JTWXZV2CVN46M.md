When we go to merge an issue, we need to verify that we are on an issue branch, if not we need to delegate to the abort tool.

## Proposed Solution

I need to add branch validation to the issue merge tool to ensure it only runs when we're on an issue branch. When not on an issue branch, it should delegate to the abort tool.

### Implementation Steps

1. **Add branch validation in the merge tool**: Before attempting to merge, check if we're currently on an issue branch using `git_ops.current_branch()` and `git_ops.is_issue_branch()`

2. **Delegate to abort tool**: If not on an issue branch, use the context's tool execution mechanism to call `abort_create` with an appropriate reason

3. **Error handling**: Ensure proper error handling and logging for both branch validation and abort delegation

### Code Changes

In `/Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`:

- Add branch validation after parsing arguments but before issue validation
- Check current branch using `git_ops.current_branch()`
- Use `is_issue_branch()` method to validate we're on an issue branch
- If not on issue branch, call abort tool with context about invalid branch state
- Only proceed with existing merge logic if validation passes

This follows the existing patterns in the codebase and ensures merge operations only happen from the correct context.