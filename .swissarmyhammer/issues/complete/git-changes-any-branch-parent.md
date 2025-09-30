# Remove issue/ Prefix Requirement from Git Changes Tool

## Problem

Currently, the `git_changes` tool only performs parent branch detection for branches with the `issue/` prefix. This is unnecessarily restrictive - we should detect parent branches for **any** branch that has one, regardless of naming convention.

## Current Behavior

- Branches starting with `issue/`: Uses merge-base to find parent branch
- All other branches (including `main`): Returns all tracked files

## Desired Behavior

- **Any branch with a parent**: Use merge-base to detect parent and return files changed since divergence
- **Root branches without parents** (like `main`, `develop`): Return all tracked files

## Implementation Approach

Replace the prefix check with actual parent branch detection:

1. For any given branch, attempt to find its upstream tracking branch or common ancestors
2. Use git commands to determine if the branch has diverged from another branch
3. If a parent/base branch is found, use merge-base to calculate changes
4. If no parent exists (true root branch), return all tracked files

## Benefits

- Works with any branching strategy (feature/, bugfix/, hotfix/, or no prefix)
- More intuitive behavior aligned with actual git relationships
- Reduces coupling to specific naming conventions
- Better supports diverse team workflows

## Related

This enhances the git changes tool to be more universally applicable across different project conventions.


## Proposed Solution

After analyzing the code, I found that:

1. **Current Implementation** (lines 120-128 in `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`):
   - Only calls `find_merge_target_for_issue()` when branch starts with `issue/`
   - For all other branches, returns all tracked files

2. **The `find_merge_target_for_issue()` function** is actually generic and works with any `BranchName`:
   - It finds merge-base with all local branches (except other issue/ branches)
   - Uses git's merge-base algorithm to determine the parent
   - Returns the branch with the best merge-base match

3. **Solution**: Remove the `starts_with("issue/")` check and call `find_merge_target_for_issue()` for ANY branch
   - This will automatically:
     - Return a parent for branches that diverged from another branch
     - Return None for root branches (like main) that have no parent
   - We need to update `find_merge_target_for_issue()` to not skip non-issue branches in its candidate search

### Implementation Steps

1. Write a failing test for a `feature/` branch that should detect its parent
2. Remove the `if request.branch.starts_with("issue/")` check in the git changes tool
3. Update `find_merge_target_for_issue()` to not filter out non-issue branches as candidates
4. Verify all existing tests still pass
5. Verify the new test passes

### Key Insight

The function name `find_merge_target_for_issue` is misleading - it actually works for any branch. We might rename it to `find_parent_branch()` for clarity, but that's optional for this issue.



## Implementation Notes

### Changes Made

1. **Modified `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`**:
   - Removed the `if request.branch.starts_with("issue/")` check (lines 120-128)
   - Now calls `find_merge_target_for_issue()` for ANY branch
   - Added logic to filter out cases where the returned parent is the same as the branch itself
   - This ensures that root branches (like main) get `None` as parent instead of themselves

2. **Modified `swissarmyhammer-git/src/operations.rs`**:
   - Updated `find_merge_target_for_issue()` to be truly generic for any branch
   - Changed candidate filtering logic from hardcoded `issue/` check to dynamic prefix detection
   - Now skips branches with the same prefix as the current branch (e.g., `issue/` branches skip other `issue/` branches, `feature/` branches skip other `feature/` branches)
   - Added logic to distinguish between:
     - Branches with no candidates (fall back to main)
     - Branches with candidates but no valid merge-base (orphan/unrelated branches - return error)
     - Main branch querying itself (return error)

3. **Added Test**:
   - Created `test_git_changes_tool_feature_branch_detects_parent()` to verify `feature/` prefix branches work correctly

### Test Results

All 15 git changes tests pass:
- ✅ New test: `test_git_changes_tool_feature_branch_detects_parent`
- ✅ Existing tests still pass including orphan branch and main branch tests

### Behavior Changes

**Before**: Only `issue/*` branches detected parents; all others returned all tracked files

**After**: 
- Any branch with a divergence point detects its parent automatically
- Works with any prefix: `feature/`, `bugfix/`, `hotfix/`, or no prefix at all
- Root branches (main, develop) correctly return `None` as parent
- Orphan branches correctly return `None` as parent

The tool is now naming-convention agnostic and works based on actual git relationships.



## Code Review Fixes Applied

### Summary
Completed code review fixes addressing logging standards, documentation, and test clarification. All git-related tests pass (27/27), and clippy reports no warnings.

### Changes Applied

1. **Replaced `eprintln!` with `debug!` logging** (swissarmyhammer-git/src/operations.rs:740-897)
   - Replaced 11 `eprintln!` statements with `debug!()` macro from tracing crate
   - This aligns with project coding standards requiring tracing over eprintln
   - Locations fixed:
     - Function entry logging
     - Candidate branch discovery
     - Merge-base analysis logging
     - Score calculation logging
     - Final target selection logging

2. **Added inline documentation for branch prefix logic** (swissarmyhammer-git/src/operations.rs:769-772)
   - Added comprehensive comment explaining why we extract prefixes
   - Clarifies the sibling branch filtering algorithm
   - Explains the goal: finding parent from different hierarchy level (e.g., feature/foo → main, not feature/foo → feature/bar)

3. **Documented debug_branch_creation_issue test** (swissarmyhammer-git/src/operations.rs:987-989)
   - Added doc comment explaining test purpose
   - Clarifies it validates the business rule preventing issue branches from being created from other issue branches
   - Test is legitimate and should be kept

### Test Results

**Git Package Tests**: ✅ 16/16 passed
**Git-related Tool Tests**: ✅ 27/27 passed (1 leaky)
**Clippy**: ✅ No warnings or errors

### Code Quality Improvements

- ✅ All logging now uses tracing infrastructure
- ✅ Complex algorithms have clear documentation
- ✅ Test purposes are documented
- ✅ Code follows project standards

### Files Modified

1. `swissarmyhammer-git/src/operations.rs` - Fixed logging and added documentation
2. `CODE_REVIEW.md` - Removed after addressing all issues
