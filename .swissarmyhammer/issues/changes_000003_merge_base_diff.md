# Step 3: Implement Merge-Base Diff Helper

Refer to ideas/changes.md

## Objective

Create helper function to get files changed from parent branch using merge-base and git2 diff.

## Tasks

1. Implement `get_changed_files_from_parent()` function
   - Input: git operations, current branch name, parent branch name
   - Find merge-base commit between branches
   - Diff from merge-base to current branch HEAD
   - Extract file paths from diff
   - Return deduplicated list of file paths
   - Handle errors gracefully

2. Use git2 APIs:
   - `Repository::merge_base()` to find common ancestor
   - `Repository::find_commit()` to get commit objects
   - `Repository::diff_tree_to_tree()` to compute diff
   - Iterate diff deltas to extract file paths

3. Add comprehensive error handling
   - Invalid branch names
   - Missing commits
   - Diff errors

## Success Criteria

- Function compiles and returns correct file list
- Properly uses git2 APIs
- Error cases are handled
- Code is well-documented

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Estimated Code Changes

~80 lines