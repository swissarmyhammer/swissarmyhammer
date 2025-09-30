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

## Proposed Solution

After analyzing the existing code, I will implement a `get_uncommitted_changes()` helper function that:

1. **Function Signature**:
   ```rust
   pub fn get_uncommitted_changes(git_ops: &GitOperations) -> Result<Vec<String>>
   ```

2. **Implementation Strategy**:
   - Call `git_ops.get_status()` to retrieve the `StatusSummary`
   - Use `StatusSummary::all_changed_files()` to get staged/modified/deleted files
   - Include `untracked` files by also accessing `summary.untracked`
   - Combine and deduplicate all file paths
   - Return sorted list for consistent output

3. **Key Design Decisions**:
   - Leverage existing `StatusSummary` infrastructure rather than duplicating git2 status calls
   - Use `all_changed_files()` which already covers: staged_modified, unstaged_modified, staged_new, staged_deleted, unstaged_deleted, renamed
   - Add untracked files separately since `all_changed_files()` excludes them
   - Return `Result<Vec<String>>` to propagate errors from git operations

4. **Test Coverage**:
   - Test with clean repository (empty result)
   - Test with staged files
   - Test with unstaged modifications
   - Test with untracked files
   - Test with combination of all change types
   - Test error handling when git operations fail

5. **Files to Modify**:
   - `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`: Add helper function and tests

Estimated implementation: ~50 lines including comprehensive tests
## Implementation Notes

Successfully implemented `get_uncommitted_changes()` helper function with the following details:

### Implementation
- **Location**: `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs:55-68`
- **Function Signature**: `pub fn get_uncommitted_changes(git_ops: &GitOperations) -> GitResult<Vec<String>>`
- **Lines of Code**: 14 lines (excluding documentation)

### Key Design Choices
1. Used existing `GitOperations::get_status()` method to retrieve status
2. Called `StatusSummary::all_changed_files()` which includes:
   - staged_modified
   - unstaged_modified
   - staged_new
   - staged_deleted
   - unstaged_deleted
   - renamed
3. Manually added `untracked` files since `all_changed_files()` excludes them by design
4. Applied deduplication and sorting for consistent output
5. Proper error propagation via `GitResult<Vec<String>>`

### Test Coverage
Implemented 5 comprehensive tests (lines 122-358):
1. `test_get_uncommitted_changes_clean_repo` - verifies empty result for clean repo
2. `test_get_uncommitted_changes_staged_files` - validates staged file detection
3. `test_get_uncommitted_changes_unstaged_modifications` - validates unstaged changes
4. `test_get_uncommitted_changes_untracked_files` - validates untracked file detection
5. `test_get_uncommitted_changes_mixed_changes` - validates combination of all change types and sorted output

### Build and Test Results
- ✅ Compilation successful with `cargo build`
- ✅ All 11 tests pass with `cargo nextest`
- ✅ Code formatted with `cargo fmt`

### Dependencies
- Added imports: `swissarmyhammer_git::{GitOperations, GitResult}`
- Test dependencies: `tempfile::TempDir` (already in dev-dependencies)

The implementation is complete, tested, and ready for integration into the larger git_changes tool.

## Execute Method Implementation

Successfully implemented the `execute()` method for the GitChangesTool (lines 104-164):

### Implementation Details

1. **Request Parsing**: Deserializes `GitChangesRequest` from MCP arguments
2. **Git Operations Access**: Retrieves `GitOperations` from the tool context's shared state
3. **Parent Branch Detection**: 
   - For issue branches (starting with "issue/"): Uses `find_merge_target_for_issue()` to find parent
   - For other branches: Treats as main/trunk branches (no parent)
4. **File Collection**:
   - With parent: Calls `get_changed_files_from_parent()` to get diff from parent
   - Without parent: Calls `get_all_tracked_files()` to get all tracked files
5. **Response Generation**: Creates `GitChangesResponse` with branch name, optional parent, and file list

### Design Decisions

- Used context's shared `git_ops` instead of creating new instance
- Graceful fallback: If parent detection fails for issue branch, treat as main branch
- Proper MCP response structure with `CallToolResult` including content, error flag, and metadata

### Test Coverage

Added 3 comprehensive integration tests (lines 451-644):

1. `test_git_changes_tool_execute_main_branch`: Tests main branch returns all tracked files
2. `test_git_changes_tool_execute_issue_branch`: Tests issue branch returns only changed files from parent
3. `test_git_changes_tool_execute_no_git_ops`: Tests error handling when git ops unavailable

### Test Refactoring

Created helper functions to eliminate duplicate test setup code:
- `setup_test_repo()`: Initializes git repo with user config (reduces ~12 lines per test)
- `create_initial_commit()`: Creates and commits initial file (reduces ~8 lines per test)

Applied helpers across all 5 uncommitted changes tests and 2 tool execution tests, reducing total test code by ~100 lines while maintaining full coverage.

### Build and Test Results

- ✅ All 8 tests passing with `cargo nextest`
- ✅ Compilation successful with no warnings
- ✅ Code formatted with `cargo fmt`

The git_changes tool is now fully functional and ready for use.