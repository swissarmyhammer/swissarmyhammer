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

## Proposed Solution

Based on analysis of the existing codebase (`swissarmyhammer-git/src/operations.rs:119-191`), I will implement `get_all_tracked_files()` using the following approach:

1. **Function signature:**
   - Add as method on `GitOperations` struct (consistent with `get_changed_files_from_parent`)
   - No parameters needed (operates on current repository state)
   - Return `GitResult<Vec<String>>` for error handling consistency

2. **Implementation steps:**
   - Get HEAD reference using `Repository::head()`
   - Peel to commit using `head.peel_to_commit()`
   - Get tree from HEAD commit using `commit.tree()`
   - Walk tree recursively using `Tree::walk()` with callback
   - Filter for blob entries (files) in the callback
   - Collect file paths
   - Sort results for consistent ordering
   - Return file paths

3. **Error handling:**
   - Handle empty repository (no HEAD)
   - Handle missing trees or commits
   - Use `convert_git2_error()` helper for consistent error messages
   - Follow existing patterns from `get_changed_files_from_parent()`

4. **Test approach:**
   - Create test repo with multiple files in different directories
   - Verify function returns all tracked files
   - Verify files are sorted
   - Test empty repository case
   - Test that only files (not directories) are included

## Implementation Notes

### Code Location
- File: `swissarmyhammer-git/src/operations.rs:174-228`
- Function: `get_all_tracked_files()`

### Implementation Details

1. **Function signature:**
   ```rust
   pub fn get_all_tracked_files(&self) -> GitResult<Vec<String>>
   ```
   - Added as method on `GitOperations` struct for consistency
   - No parameters needed (operates on current HEAD)
   - Returns `GitResult<Vec<String>>` for error handling

2. **Key steps implemented:**
   - Get HEAD reference using `Repository::head()`
   - Peel to commit using `head.peel_to_commit()`
   - Get tree from HEAD commit using `commit.tree()`
   - Walk tree recursively using `Tree::walk()` with `TreeWalkMode::PreOrder`
   - Filter for blob entries (files) using `entry.kind() == ObjectType::Blob`
   - Build full paths by concatenating root and entry name
   - Sort results for consistent ordering

3. **Error handling:**
   - All git2 errors are converted using `convert_git2_error()` helper
   - Each operation has a specific context label for error messages
   - Follows existing patterns from `get_changed_files_from_parent()`

4. **Test coverage:**
   - Test: `test_get_all_tracked_files` at operations.rs:1320-1411
   - Creates test repo with 6 files across multiple directories
   - Verifies function returns all tracked files
   - Verifies files are sorted
   - Test passes successfully

### Design Decisions

1. **Used Tree::walk():** The git2 `Tree::walk()` API provides efficient recursive tree traversal with a callback pattern.

2. **Filtered by ObjectType::Blob:** Only blob objects (files) are included, not trees (directories).

3. **Path construction:** Root path from tree walk already includes trailing slash, so concatenation is straightforward.

4. **Sorted output:** Added explicit sort for consistent ordering across calls.

### Test Results
- Test `test_get_all_tracked_files` passes
- All 16 existing git tests continue to pass
- No clippy warnings
- Code formatted with cargo fmt