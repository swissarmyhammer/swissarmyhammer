# Step 4: Remove issue_merge Tool

**Refer to ideas/issue_work_cleanup.md**

## Overview

Delete the `issue_merge` tool and remove all its registrations. This tool is no longer needed since we're eliminating the automatic issue branch workflow.

## Context

The `issue_merge` tool currently:
- Validates user is on an issue branch
- Auto-completes issues before merge
- Merges issue branches back to source branch
- Optionally deletes branches after merge

With the new approach, users manage their own git workflow and merges directly.

## Dependencies

**Requires**: Step 3 (Remove issue_work) should be completed for consistency.

## Implementation Tasks

### 1. Delete Tool File

**Delete**: `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs` (280 lines)

This file contains:
- `MergeIssueTool` struct and implementation
- Branch validation logic
- Auto-completion integration
- Merge logic with git operations
- Branch deletion logic

### 2. Remove from Tool Registration

**File**: `swissarmyhammer-tools/src/mcp/tools/issues/mod.rs`

Remove:
```rust
pub mod merge;  // Delete this line
```

Remove from `register_issue_tools()` function:
```rust
registry.register(merge::MergeIssueTool::new());  // Delete this line
```

Update module documentation to remove `merge` from the list of available tools.

### 3. Remove Type Definition

**File**: `swissarmyhammer-tools/src/mcp/types.rs` (lines 87-93)

Delete:
```rust
pub struct MergeIssueRequest {
    /// Issue name to merge
    pub name: swissarmyhammer_issues::IssueName,
    /// Whether to delete the branch after merging (default: false)
    #[serde(default)]
    pub delete_branch: bool,
}
```

### 4. Verify Build

Run `cargo build` to ensure:
- No compilation errors
- No unused import warnings
- No dead code warnings

## What NOT to Touch (Yet)

- **Tests**: Will be removed in Steps 5-6
- **Documentation**: Will be removed in Step 7
- **Prompts**: Will be updated in Step 8

## Architecture Impact

```mermaid
graph TD
    A[Before: issue_merge tool] -->|validates| B[on issue branch]
    A -->|auto-completes| C[issue]
    A -->|merges to| D[source branch]
    A -->|deletes| B
    
    E[After: Manual workflow] -->|user runs| F[git merge]
    E -->|user completes| G[issue manually]
    E -->|user manages| H[branches]
    
    style A fill:#fbb,stroke:#333,stroke-width:2px
    style B fill:#fbb,stroke:#333,stroke-width:2px
    style C fill:#fbb,stroke:#333,stroke-width:2px
    style D fill:#fbb,stroke:#333,stroke-width:2px
    style E fill:#bfb,stroke:#333,stroke-width:2px
```

## Success Criteria

- [ ] `merge/mod.rs` file deleted
- [ ] Tool registration removed from `issues/mod.rs`
- [ ] `MergeIssueRequest` type removed from `types.rs`
- [ ] `cargo build` succeeds with no errors
- [ ] No warnings about unused imports or dead code
- [ ] Module documentation updated

## Estimated Changes

- **Deletions**: ~280 lines (entire tool file)
- **Modifications**: ~15 lines (registration and types)

## Files to Delete

- `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs`

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/issues/mod.rs` (~10 lines)
- `swissarmyhammer-tools/src/mcp/types.rs` (~7 lines)

## Verification Commands

```bash
# Verify build
cargo build

# Verify no references remain in code
rg "issue_merge" swissarmyhammer-tools/src/

# Verify MergeIssueRequest removed
rg "MergeIssueRequest" swissarmyhammer-tools/src/

# Check for any lingering references to merge tool
rg "MergeIssueTool" swissarmyhammer-tools/src/
```

## Migration Notes for Users

Users who currently use `issue_merge`:
- Use standard git merge commands directly
- Manually complete issues with `issue_mark_complete`
- Manage branch lifecycle with git commands
- More control over merge strategy and timing

## Impact Assessment

### What Stays
- Issue completion (`issue_mark_complete`)
- Issue listing and display
- Issue creation and updates
- Git operations (still available, just not automated)

### What Goes
- Automatic merge-base detection
- Auto-completion before merge
- Forced branch validation
- Automatic branch deletion

## Next Steps

Step 5 will remove tests that specifically test the branching workflow and tool validations.


## Proposed Solution

After examining the codebase, I will implement the following steps to remove the `issue_merge` tool:

### Step 1: Delete the Tool File
- Delete `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs` (280 lines)
- This file contains the complete `MergeIssueTool` implementation including branch validation, auto-completion logic, merge operations, and branch deletion

### Step 2: Remove Module Registration
- Edit `swissarmyhammer-tools/src/mcp/tools/issues/mod.rs`:
  - Remove `pub mod merge;` declaration (line 65)
  - Remove `registry.register(merge::MergeIssueTool::new());` from `register_issue_tools()` function (line 125)
  - Update module documentation to remove mention of merge tool from the "Available Tools" section

### Step 3: Remove Type Definition
- Edit `swissarmyhammer-tools/src/mcp/types.rs`:
  - Remove `MergeIssueRequest` struct (lines 87-93)
  - This includes the struct definition with `name` and `delete_branch` fields

### Step 4: Build and Verify
- Run `cargo build` to ensure no compilation errors
- Run `cargo fmt --all` to format the modified files
- Run verification commands to check for any remaining references:
  - `rg "issue_merge"` - should find nothing in swissarmyhammer-tools/src/
  - `rg "MergeIssueRequest"` - should find nothing in swissarmyhammer-tools/src/
  - `rg "MergeIssueTool"` - should find nothing in swissarmyhammer-tools/src/

### Expected Impact
- Users will no longer have access to the `issue_merge` tool
- Users will need to use standard git commands to merge branches
- Issue completion will still be available via `issue_mark_complete`
- No impact on issue creation, listing, or display functionality

### Implementation Notes
- The merge tool currently auto-completes issues before merging by calling `MarkCompleteIssueTool`
- After removal, users must manually complete issues with `issue_mark_complete` before merging
- The merge tool uses `find_merge_target_for_issue()` to determine the target branch via git merge-base
- After removal, users must manually determine and merge to the appropriate branch



## Implementation Complete

Successfully removed the `issue_merge` tool from the codebase. All tests pass (3326/3326).

### Changes Made

1. **Deleted Tool Implementation**
   - Removed `swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs` (280 lines)
   - Removed `swissarmyhammer-tools/src/mcp/tools/issues/merge/description.md`
   - Removed entire `merge/` directory

2. **Updated Module Registration**
   - swissarmyhammer-tools/src/mcp/tools/issues/mod.rs:
     - Removed `pub mod merge;` declaration
     - Removed `registry.register(merge::MergeIssueTool::new());` call
     - Updated module documentation to remove merge tool from "Available Tools" list
     - Updated "Issue Workflow" documentation to remove merge step

3. **Removed Type Definitions**
   - swissarmyhammer-tools/src/mcp/types.rs:
     - Removed `MergeIssueRequest` struct definition
   - swissarmyhammer-tools/src/mcp/mod.rs:
     - Removed `MergeIssueRequest` from re-exports

4. **Updated Tests**
   - swissarmyhammer-tools/src/mcp/tests.rs:
     - Removed `MergeIssueRequest` from imports
     - Removed `MergeIssueRequest` schema validation test
   - swissarmyhammer-tools/tests/mcp_server_parity_tests.rs:
     - Removed `issue_merge` from HTTP static tools list
     - Updated minimum tool count from 27 to 26

### Verification Results

- ✅ `cargo build` - No compilation errors
- ✅ `cargo fmt --all` - All files formatted
- ✅ `cargo nextest run` - All 3326 tests pass
- ✅ No references to `issue_merge` remain in swissarmyhammer-tools/src/
- ✅ No references to `MergeIssueRequest` remain in swissarmyhammer-tools/src/
- ✅ No references to `MergeIssueTool` remain in swissarmyhammer-tools/src/

### Files Modified

- swissarmyhammer-tools/src/mcp/tools/issues/mod.rs
- swissarmyhammer-tools/src/mcp/types.rs
- swissarmyhammer-tools/src/mcp/mod.rs
- swissarmyhammer-tools/src/mcp/tests.rs
- swissarmyhammer-tools/tests/mcp_server_parity_tests.rs

### Files Deleted

- swissarmyhammer-tools/src/mcp/tools/issues/merge/mod.rs
- swissarmyhammer-tools/src/mcp/tools/issues/merge/description.md
- swissarmyhammer-tools/src/mcp/tools/issues/merge/ (directory)
