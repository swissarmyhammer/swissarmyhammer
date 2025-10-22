# Fix Git Changes Tool Behavior on Main/Trunk Branches

## Problem

Currently, the `git_changes` MCP tool returns **all tracked files** when executed on main/trunk branches. This behavior is problematic for work-in-progress checking scenarios where we only want to see uncommitted changes.

## Current Behavior

```rust
// When on main/trunk branches
if is_main_or_trunk(branch) {
    return all_tracked_files(); // ❌ Returns everything
}
```

## Required New Behavior

```rust
// When on main/trunk branches
if is_main_or_trunk(branch) {
    return uncommitted_changes_only(); // ✅ Returns only WIP
}
```

### Detailed Behavior

**Feature Branch** (no change):
- Returns all files changed since branch diverged from parent
- Includes both committed and uncommitted changes on the branch

**Main/Trunk Branch** (changed):
- Returns only uncommitted changes (staged + unstaged)
- Does NOT return all tracked files
- Useful for checking WIP before committing

## Implementation

### 1. Add `get_uncommitted_changes()` Function

Location: `swissarmyhammer-git/src/lib.rs` (or wherever git operations live)

```rust
/// Get uncommitted changes (both staged and unstaged)
pub fn get_uncommitted_changes() -> Result<Vec<PathBuf>> {
    let mut files = HashSet::new();
    
    // Get unstaged changes
    let unstaged = Command::new("git")
        .args(["diff", "HEAD", "--name-only"])
        .output()?;
    
    // Get staged changes
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .output()?;
    
    // Parse and combine both
    for line in String::from_utf8_lossy(&unstaged.stdout).lines() {
        files.insert(PathBuf::from(line.trim()));
    }
    
    for line in String::from_utf8_lossy(&staged.stdout).lines() {
        files.insert(PathBuf::from(line.trim()));
    }
    
    Ok(files.into_iter().collect())
}
```

### 2. Update `git_changes` Tool Logic

Modify the branch detection logic:

```rust
pub fn get_changed_files(branch: &str) -> Result<Vec<PathBuf>> {
    if is_main_or_trunk_branch(branch) {
        // NEW: Return only uncommitted changes
        get_uncommitted_changes()
    } else {
        // EXISTING: Return all changes since branch divergence
        get_branch_changes_since_divergence(branch)
    }
}

fn is_main_or_trunk_branch(branch: &str) -> bool {
    matches!(branch, "main" | "master" | "trunk" | "develop")
}
```

### 3. Update MCP Tool

Location: `swissarmyhammer-tools/src/mcp/tools/git/changes.rs`

Update the MCP tool wrapper to use the new library behavior.

## Files to Modify

1. `swissarmyhammer-git/src/lib.rs`
   - Add `get_uncommitted_changes()`
   - Modify `get_changed_files()` logic
   - Add `is_main_or_trunk_branch()` helper

2. `swissarmyhammer-tools/src/mcp/tools/git/changes.rs`
   - Update to use modified library function
   - Update documentation

3. `swissarmyhammer-cli/src/commands/git/changes.rs` (if exists)
   - Ensure CLI uses updated behavior

## Tests

1. **Test on feature branch**: Returns all changes since divergence (existing behavior)
   ```bash
   git checkout -b feature/test
   # Make changes and commit
   git_changes("feature/test") // Should return all changes since branching
   ```

2. **Test on main with no changes**: Returns empty list
   ```bash
   git checkout main
   git_changes("main") // Should return []
   ```

3. **Test on main with staged changes**: Returns staged files
   ```bash
   git checkout main
   echo "test" > file.txt
   git add file.txt
   git_changes("main") // Should return ["file.txt"]
   ```

4. **Test on main with unstaged changes**: Returns unstaged files
   ```bash
   git checkout main
   echo "test" > file.txt
   git_changes("main") // Should return ["file.txt"]
   ```

5. **Test on main with both staged and unstaged**: Returns union
   ```bash
   git checkout main
   echo "1" > file1.txt
   git add file1.txt
   echo "2" > file2.txt
   git_changes("main") // Should return ["file1.txt", "file2.txt"]
   ```

## Success Criteria

- ✅ On feature branches: returns all changes since divergence (no change)
- ✅ On main/trunk: returns only uncommitted changes (staged + unstaged)
- ✅ Never returns "all tracked files" on main/trunk
- ✅ All tests pass
- ✅ MCP tool documentation updated

## Why This is First

This issue must be completed **before** `add-changed-filter-to-rule-check` because:
1. The `--changed` filter depends on `git_changes` returning the correct set of files
2. Without this fix, `--changed` on main would try to check all tracked files
3. This is a focused change with clear scope and tests

## Related Issues

- Blocks: `add-changed-filter-to-rule-check`



## Proposed Solution

After analyzing the code, I've identified the exact problem:

**Current Implementation** (lines 135-150 in `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`):
```rust
let mut files = if let Some(ref parent) = parent_branch {
    // Feature/issue branch: get files changed from parent
    git_ops.get_changed_files_from_parent(&request.branch, parent).map_err(...)?
} else {
    // Main/trunk branch: get all tracked files  ❌ THIS IS THE PROBLEM
    git_ops.get_all_tracked_files().map_err(...)?
};
```

**The Fix**:
Replace the `else` branch to use `get_uncommitted_changes` instead of `get_all_tracked_files`:

```rust
let mut files = if let Some(ref parent) = parent_branch {
    // Feature/issue branch: get files changed from parent
    git_ops.get_changed_files_from_parent(&request.branch, parent).map_err(...)?
} else {
    // Main/trunk branch: get only uncommitted changes ✅ FIXED
    get_uncommitted_changes(git_ops).map_err(...)?
};
```

**Why this works**:
1. The `get_uncommitted_changes` function already exists and is working correctly (lines 55-69)
2. It returns staged + unstaged + untracked files (exactly what we want)
3. For feature branches, we still get the full diff from parent PLUS uncommitted changes (lines 152-159)
4. For main/trunk, we now get ONLY uncommitted changes

**Test Updates Required**:
1. `test_git_changes_tool_execute_main_branch` (line 319) - needs to expect only uncommitted files, not all tracked files
2. `test_git_changes_tool_main_branch_includes_uncommitted` (line 500) - needs adjustment to expect only uncommitted files
3. `test_git_changes_tool_invalid_branch` (line 545) - needs adjustment since non-existent branches will now return uncommitted changes

**Implementation Steps**:
1. ✅ Read and understand the current code
2. Update the execute method logic (one line change)
3. Update failing tests to match new behavior
4. Run tests to verify
5. Update documentation if needed




## Implementation Complete

### Changes Made

**1. Core Logic Fix** (`swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs:145-153`)
- Changed main/trunk branch behavior from `get_all_tracked_files()` to `get_uncommitted_changes()`
- This ensures main/trunk branches return only uncommitted changes (staged + unstaged + untracked)

**2. Documentation Update** (lines 1-12)
- Updated module-level documentation to reflect new behavior
- Main/trunk branches: "Only uncommitted changes (staged + unstaged + untracked)"
- Feature/Issue branches: "Files changed since diverging from the parent branch (plus uncommitted changes)"

**3. Test Updates**
- `test_git_changes_tool_execute_main_branch` (line 321): Now expects 0 files when no uncommitted changes
- `test_git_changes_tool_main_branch_includes_uncommitted` (line 501): Expects only 1 uncommitted file, not all 3 tracked files
- `test_git_changes_tool_invalid_branch` (line 547): Updated comment and expectations for uncommitted-only behavior

### Test Results
✅ All 14 git_changes tests passing
✅ Full project builds successfully
✅ Code formatted with cargo fmt

### Behavior Verification

**Before Fix:**
- Main branch: Returns all tracked files in repository (❌ wrong)
- Feature branch: Returns changed files since divergence + uncommitted

**After Fix:**
- Main branch: Returns only uncommitted changes (✅ correct)
- Feature branch: Returns changed files since divergence + uncommitted (✅ unchanged)

### Files Modified
1. `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs` - Core implementation and tests

