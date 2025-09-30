# Step 4: Implement Get All Tracked Files Helper

Refer to ideas/changes.md

## Objective

Create helper function to get all tracked files in the repository (for root branches).

## Tasks

1. Implement `get_all_tracked_files()` function
   - Input: git operations
   - Get HEAD commit
   - Get tree for HEAD commit
   - Walk tree recursively
   - Collect all file paths (not directories)
   - Return sorted list of file paths
   - Handle errors gracefully

2. Use git2 APIs:
   - `Repository::head()` to get HEAD reference
   - `Commit::tree()` to get commit tree
   - `Tree::walk()` or manual iteration to traverse tree
   - Filter for blob entries (files)

3. Add comprehensive error handling
   - Empty repository
   - Missing HEAD
   - Tree traversal errors

## Success Criteria

- Function compiles and returns complete file list
- Properly uses git2 APIs
- Only includes files, not directories
- Error cases are handled
- Code is well-documented

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Estimated Code Changes

~60 lines