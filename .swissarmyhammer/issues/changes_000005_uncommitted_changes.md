# Step 5: Implement Uncommitted Changes Helper

Refer to ideas/changes.md

## Objective

Create helper function to get uncommitted changes (staged and unstaged).

## Tasks

1. Implement `get_uncommitted_changes()` function
   - Input: git operations
   - Use existing `GitOperations::get_status()` method
   - Extract file paths from status summary
   - Combine staged, unstaged, and untracked files
   - Return deduplicated list of file paths
   - Handle errors gracefully

2. Use existing infrastructure:
   - `GitOperations::get_status()` returns `StatusSummary`
   - `StatusSummary::all_changed_files()` provides file list
   - No need to directly use git2 status APIs

3. Add error handling
   - Status query failures
   - Invalid repository state

## Success Criteria

- Function compiles and returns uncommitted file list
- Reuses existing GitOperations methods
- Properly combines all types of changes
- Error cases are handled
- Code is well-documented

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Estimated Code Changes

~40 lines