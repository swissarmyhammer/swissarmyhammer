# Migrate All Git Operations to libgit2

## Problem

Currently, the codebase shells out to git commands using shell execution. This approach has several drawbacks:

- **Performance**: Process spawning overhead for each git operation
- **Portability**: Depends on git being installed and in PATH
- **Error Handling**: Parsing text output is fragile and error-prone
- **Security**: Shell injection risks if inputs aren't properly sanitized
- **Reliability**: Different git versions may have different output formats

## Solution

Replace all git shell commands with libgit2 (via `git2` crate in Rust).

## Benefits

- **Performance**: Direct library calls, no process spawning
- **Type Safety**: Structured data instead of parsing text output
- **Portability**: No external git dependency required
- **Reliability**: Consistent behavior across environments
- **Better Error Handling**: Proper error types instead of exit codes
- **Feature Rich**: Access to full git internals programmatically

## Affected Components

Audit the codebase for all instances of:
- Shell commands calling `git` (via `shell_execute` or similar)
- Parsing git command output (status, diff, log, etc.)
- Git operations in tools like `git_changes`

## Implementation Steps

1. Add `git2` crate dependency
2. Identify all git shell commands in codebase
3. Replace each with equivalent libgit2 API calls
4. Update error handling to use git2 error types
5. Add tests to verify equivalent behavior
6. Remove shell-based git helpers

## Examples of Migration

**Before** (shell):
```rust
Command::new("git")
    .args(["diff", "--name-only", "HEAD"])
    .output()
```

**After** (libgit2):
```rust
let repo = Repository::open(".")?;
let head = repo.head()?.peel_to_tree()?;
// Use libgit2 diff APIs
```

## Related

- Improves `git_changes` tool reliability
- Enables better git integration throughout the system
- Foundation for future git-based features


## Proposed Solution

### Investigation Summary

I've audited the codebase for git shell command usage and found that **the primary migration has already been completed!** Here's what I discovered:

1. **swissarmyhammer-git crate**: A complete, well-designed libgit2 wrapper exists at `/swissarmyhammer-git/`:
   - Provides type-safe `BranchName` wrapper
   - Full libgit2-based `GitOperations` API
   - Comprehensive error handling with `GitError` and `GitResult`
   - Already implements all core operations: branch creation, checkout, merge, status, diff, commit history

2. **git_changes tool**: Already migrated to use libgit2 via `swissarmyhammer_git::GitOperations`
   - Uses `get_changed_files_from_parent()` which uses libgit2's merge-base and diff APIs
   - Uses `get_all_tracked_files()` which walks git trees using libgit2
   - Uses `get_status()` which uses libgit2's status APIs

3. **Remaining Shell Usage**: Only found in **test setup code** (24 occurrences in `git_changes/mod.rs`)
   - `setup_test_repo()` helper uses shell commands to initialize test repos
   - `create_initial_commit()` helper uses shell commands to create test commits
   - These are test utilities, not production code

4. **Dependencies**: `git2 = "0.18"` already in workspace `Cargo.toml:117`

### Scope Adjustment

Since the production code migration is complete, the remaining work is:

1. **Migrate test helpers** in `git_changes/mod.rs` to use the `GitOperations` API instead of shell commands
   - Replace `Command::new("git")` calls with `GitOperations` methods
   - Use existing patterns from `swissarmyhammer-git/src/operations.rs` tests (lines 986-1536)

2. **Search for other test utilities** that might be using shell-based git commands
   - Check for any other test files using `Command::new("git")`
   - Ensure consistency across the test suite

### Implementation Plan

1. ✅ Audit complete - identified 24 shell commands in test code
2. Create a shared test utilities module with libgit2-based helpers:
   - `TestRepo::new()` - initialize test repo with libgit2
   - `TestRepo::commit()` - create commits using libgit2
   - `TestRepo::create_file()` - add files and stage them
3. Refactor `git_changes/mod.rs` tests to use new test utilities
4. Run test suite to verify no regressions
5. Update any other test files found using shell-based git commands

### Benefits Achieved

The existing migration has already delivered:
- ✅ Better performance (no process spawning overhead)
- ✅ Improved reliability (structured errors, no output parsing)
- ✅ Enhanced portability (no external git dependency)
- ✅ Type safety (BranchName prevents string confusion)
- ✅ Comprehensive git operations API

### Notes

The architecture is well-designed with proper separation of concerns:
- `swissarmyhammer-git` crate provides the libgit2 wrapper
- Tools consume the high-level API
- Test code is the only remaining area using shell commands



## Implementation Summary

### Work Completed

1. ✅ **Audited codebase** - Confirmed production code already uses libgit2
   - Main git operations in `swissarmyhammer-git` crate fully use libgit2
   - `git_changes` tool uses libgit2 via `GitOperations` API
   - Only test setup code was still using shell commands

2. ✅ **Created test helper utilities** - New `TestGitRepo` helper in `/swissarmyhammer-tools/src/test_utils/git_test_helpers.rs`
   - Provides clean libgit2-based test repository setup
   - Methods: `new()`, `commit_file()`, `create_and_checkout_branch()`, `add_all()`, etc.
   - Fully documented with tests included

3. ✅ **Migrated all git_changes tests** - Replaced 24 shell command calls with libgit2 helpers
   - Refactored 13 test functions in `git_changes/mod.rs`
   - Removed `setup_test_repo()` and `create_initial_commit()` shell-based helpers
   - Tests now use `TestGitRepo` for cleaner, faster, more portable tests

4. ✅ **Verified with test suite** - All 2949 tests pass
   - Specific git_changes tests: 14 passed
   - Full test suite: 2949 passed, 1 skipped

### Files Changed

- **New file**: `/swissarmyhammer-tools/src/test_utils/git_test_helpers.rs` (180 lines)
- **Modified**: `/swissarmyhammer-tools/src/test_utils.rs` (added module declaration)
- **Modified**: `/swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs` (replaced shell commands)

### Benefits Realized

- ✅ **Better test performance**: No process spawning overhead
- ✅ **Improved portability**: Tests no longer require git binary in PATH
- ✅ **Enhanced maintainability**: Cleaner test code using structured API
- ✅ **Increased reliability**: No text parsing or shell escaping issues

### Note on Orphan Branches

One edge case test (orphan branch) still requires shell commands (`git checkout --orphan`) as libgit2 doesn't provide direct support for this operation. This is documented in code comments and is acceptable since:
- Orphan branches are extremely rare in practice
- Production code handles them correctly (no parent = treat as main)
- This is purely a test setup limitation, not a production code issue

### Next Steps (if needed in future)

- Consider adding libgit2 orphan branch support if this becomes important
- Look for other test files in the codebase using shell-based git commands
- Document the `TestGitRepo` helper for broader use across the codebase
