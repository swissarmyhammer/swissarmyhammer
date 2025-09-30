# Step 6: Implement McpTool Trait

Refer to ideas/changes.md

## Objective

Implement the `McpTool` trait for `GitChangesTool` with complete execute logic.

## Tasks

1. Create `GitChangesTool` struct and implement `McpTool` trait
   - `name()` returns `"git_changes"`
   - `description()` loads from tool descriptions registry
   - `schema()` returns JSON schema for GitChangesRequest
   - `execute()` implements the main logic

2. Implement `execute()` method:
   - Parse arguments into `GitChangesRequest`
   - Access git_ops from ToolContext
   - Validate git operations are available
   - Determine parent branch using `find_merge_target_for_issue()`
   - If parent exists: call `get_changed_files_from_parent()`
   - If no parent: call `get_all_tracked_files()`
   - Get uncommitted changes and merge with committed changes
   - Deduplicate file list
   - Build `GitChangesResponse`
   - Return success response

3. Error handling:
   - Git operations not available
   - Invalid branch name
   - Git operation failures
   - Convert GitError to McpError

## Success Criteria

- Tool compiles and implements McpTool trait
- Execute logic is complete and correct
- All error cases are handled
- Proper logging with tracing
- Code is well-documented

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Estimated Code Changes

~120 lines

## Proposed Solution

After reviewing the code in `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`, I found that the `GitChangesTool` implementation is **already complete** with the following:

✅ **Already Implemented:**
1. `GitChangesTool` struct with `Default` and `new()` constructor
2. `McpTool` trait implementation with all required methods:
   - `name()` returns `"git_changes"`
   - `description()` loads from `description.md` file
   - `schema()` returns proper JSON schema for `GitChangesRequest`
   - `execute()` has complete logic:
     - Parses arguments into `GitChangesRequest`
     - Accesses git_ops from ToolContext
     - Validates git operations availability
     - Determines parent branch using `find_merge_target_for_issue()`
     - Gets changed files from parent or all tracked files
     - Builds `GitChangesResponse`
     - Returns success response
3. Error handling for all cases
4. Comprehensive test coverage (10 test functions)

❌ **Missing from Issue Requirements:**
The issue specification mentions "Get uncommitted changes and merge with committed changes" but the current `execute()` method does NOT incorporate uncommitted changes into the final file list.

### Implementation Plan

The `get_uncommitted_changes()` helper function exists and is tested (lines 55-69), but it's not being used in the `execute()` method. I need to:

1. Call `get_uncommitted_changes()` in the `execute()` method after getting committed changes
2. Merge the uncommitted files with the committed files
3. Deduplicate the combined file list
4. Add tests to verify uncommitted changes are included in the response

### Changes Required

Modify the `execute()` method around lines 133-144 to:
```rust
// Get changed files based on whether we have a parent branch
let mut files = if let Some(ref parent) = parent_branch {
    // Feature/issue branch: get files changed from parent
    git_ops
        .get_changed_files_from_parent(&request.branch, parent)
        .map_err(|e| rmcp::ErrorData::internal_error(format!("Failed to get changed files: {}", e), None))?
} else {
    // Main/trunk branch: get all tracked files
    git_ops
        .get_all_tracked_files()
        .map_err(|e| rmcp::ErrorData::internal_error(format!("Failed to get tracked files: {}", e), None))?
};

// Merge in uncommitted changes
let uncommitted = get_uncommitted_changes(git_ops)
    .map_err(|e| rmcp::ErrorData::internal_error(format!("Failed to get uncommitted changes: {}", e), None))?;
files.extend(uncommitted);

// Deduplicate and sort for consistent output
files.sort();
files.dedup();
```

This ensures the tool returns both committed and uncommitted changes, matching the issue specification.

## Implementation Summary

### Changes Made

Modified `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs:104-170` to include uncommitted changes in the tool's response.

**Key Change:**
The `execute()` method now merges uncommitted changes with committed changes before returning the file list:

```rust
// Get changed files (committed)
let mut files = if let Some(ref parent) = parent_branch {
    git_ops.get_changed_files_from_parent(&request.branch, parent)?
} else {
    git_ops.get_all_tracked_files()?
};

// Merge in uncommitted changes
let uncommitted = get_uncommitted_changes(git_ops)?;
files.extend(uncommitted);

// Deduplicate and sort for consistent output
files.sort();
files.dedup();
```

### Tests Added

Added 2 comprehensive integration tests:

1. **`test_git_changes_tool_includes_uncommitted_changes`** (lines 512-578)
   - Tests that uncommitted files are included alongside committed changes on an issue branch
   - Verifies both committed_on_branch.txt and uncommitted.txt appear in results
   - Confirms base files from parent branch are excluded

2. **`test_git_changes_tool_main_branch_includes_uncommitted`** (lines 580-634)
   - Tests that uncommitted files are included with all tracked files on main branch
   - Verifies all tracked files (file1.txt, file2.txt) plus uncommitted.txt appear in results

### Verification

- ✅ All 19 git module tests pass
- ✅ Code formatted with `cargo fmt`
- ✅ No clippy warnings (`cargo clippy -- -D warnings`)
- ✅ Build succeeds without errors

### Implementation Details

The tool now provides complete visibility into the scope of changes on any branch:
- **Issue branches**: Shows files changed from parent + uncommitted changes
- **Main/trunk branches**: Shows all tracked files + uncommitted changes

This ensures the tool accurately reflects all work in progress, both committed and uncommitted.