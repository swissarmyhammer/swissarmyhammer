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

## Proposed Solution

Based on analysis of the existing codebase, I will implement `get_changed_files_from_parent()` using the following approach:

1. **Function signature:**
   - Use `GitOperations` as the input (consistent with existing git module patterns)
   - Take current branch name and parent branch name as parameters
   - Return `Result<Vec<String>, GitError>`

2. **Implementation steps:**
   - Find branch references using `Repository::find_branch()` 
   - Get commit OIDs from branches using `branch.get().peel_to_commit()`
   - Calculate merge-base using `Repository::merge_base()`
   - Get tree objects from merge-base and current branch HEAD
   - Use `Repository::diff_tree_to_tree()` to compute diff
   - Iterate over diff deltas to extract file paths
   - Deduplicate and return file paths

3. **Pattern reference:**
   - Similar to operations.rs:680-730 which uses merge_base for branch detection
   - Will use git2::Diff APIs to extract changed files
   - Follow existing error handling patterns with `convert_git2_error`

4. **Test approach:**
   - Create test repo with main branch and feature branch
   - Make commits on both branches
   - Verify function returns files changed on feature branch since divergence

## Implementation Notes

### Code Location
- File: `swissarmyhammer-git/src/operations.rs:119-191`
- Function: `get_changed_files_from_parent()`

### Implementation Details

1. **Function signature:**
   ```rust
   pub fn get_changed_files_from_parent(
       &self,
       current_branch: &str,
       parent_branch: &str,
   ) -> GitResult<Vec<String>>
   ```

2. **Key steps implemented:**
   - Find branch references using `Repository::find_branch()` with proper error handling
   - Get commit objects by peeling branch references with `peel_to_commit()`
   - Calculate merge-base using `Repository::merge_base()` to find common ancestor
   - Get tree objects from both merge-base and current branch HEAD
   - Use `Repository::diff_tree_to_tree()` to compute the diff
   - Iterate over diff deltas to extract new file paths
   - Sort and deduplicate results (defensive, though not strictly necessary)

3. **Error handling:**
   - All git2 errors are converted using `convert_git2_error()` helper
   - Each operation has a specific context label for error messages
   - Follows existing patterns in operations.rs

4. **Test coverage:**
   - Test: `test_get_changed_files_from_parent` at operations.rs:1219
   - Creates test repo with main branch and feature branch
   - Makes multiple commits with different files on feature branch
   - Verifies function returns all 4 files changed on feature branch
   - Verifies initial commit files are not included

### Design Decisions

1. **Used existing GitOperations patterns:** Followed the same error handling and API usage patterns found in `find_merge_target_for_issue()` around line 680.

2. **Used diff deltas:** Rather than manually walking commits, used git2's diff API which is more efficient and handles edge cases.

3. **Sort and dedup:** Added defensive deduplication even though diff deltas shouldn't have duplicates - following defensive coding practices.

4. **Path extraction:** Used `delta.new_file().path()` to get the resulting path (handles renames correctly).

### Test Results
- Test passes successfully
- No warnings after fixing unused mut
- All existing tests continue to pass