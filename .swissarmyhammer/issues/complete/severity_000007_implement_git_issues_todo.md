# Step 7: Implement Severity for Git, Issues, and Todo Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for GitError, IssuesError (if exists), and TodoError.

## Context

These are domain-specific error types for git operations, issue management, and todo tracking. Each has different severity implications.

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure all three crates depend on swissarmyhammer-common:

```toml
# In swissarmyhammer-git/Cargo.toml
# In swissarmyhammer-issues/Cargo.toml  
# In swissarmyhammer-todo/Cargo.toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for GitError

In `swissarmyhammer-git/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for GitError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Git repository in bad state
            GitError::RepositoryCorrupted { .. } => ErrorSeverity::Critical,
            GitError::MergeConflict { .. } => ErrorSeverity::Critical,
            GitError::DetachedHead { .. } => ErrorSeverity::Critical,
            
            // Error: Git operation failed
            GitError::RepositoryNotFound { .. } => ErrorSeverity::Error,
            GitError::BranchNotFound { .. } => ErrorSeverity::Error,
            GitError::CommitFailed { .. } => ErrorSeverity::Error,
            GitError::InvalidReference { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical git issues
            GitError::DirtyWorkingTree { .. } => ErrorSeverity::Warning,
            GitError::UnpushedCommits { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 3. Implement Severity for TodoError

In `swissarmyhammer-todo/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for TodoError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Todo system corrupted
            TodoError::DatabaseCorrupted { .. } => ErrorSeverity::Critical,
            TodoError::FileSystemError { .. } => ErrorSeverity::Critical,
            
            // Error: Todo operation failed
            TodoError::TodoNotFound { .. } => ErrorSeverity::Error,
            TodoError::InvalidTodoFormat { .. } => ErrorSeverity::Error,
            TodoError::UpdateFailed { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            TodoError::EmptyTodo { .. } => ErrorSeverity::Warning,
            TodoError::DuplicateTodo { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 4. Check for IssuesError

Look for error types in `swissarmyhammer-issues/src/error.rs`. If found, implement Severity similarly.

### 5. Add Tests for Each Implementation

Add tests in each error.rs file:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_git_error_severity() {
        assert_eq!(
            GitError::RepositoryCorrupted { /* fields */ }.severity(),
            ErrorSeverity::Critical
        );
        
        assert_eq!(
            GitError::BranchNotFound { /* fields */ }.severity(),
            ErrorSeverity::Error
        );
        
        assert_eq!(
            GitError::DirtyWorkingTree { /* fields */ }.severity(),
            ErrorSeverity::Warning
        );
    }
}
```

## Severity Guidelines

### Git Errors
**Critical**: Repository corruption, merge conflicts, detached head
**Error**: Operations that fail but don't corrupt state
**Warning**: Informational issues (dirty tree, unpushed commits)

### Todo Errors
**Critical**: Database corruption, file system failures
**Error**: Todo operations that fail
**Warning**: Empty todos, duplicates

### Issues Errors (if applicable)
**Critical**: Issues directory corruption
**Error**: Issue operations that fail
**Warning**: Informational issues

## Acceptance Criteria

- [ ] GitError implements Severity trait
- [ ] TodoError implements Severity trait
- [ ] IssuesError implements Severity trait (if exists)
- [ ] Unit tests for each implementation
- [ ] Tests pass for all three crates
- [ ] Code compiles for all three crates
- [ ] Clippy clean for all three crates

## Files to Modify

- `swissarmyhammer-git/Cargo.toml` + `src/error.rs`
- `swissarmyhammer-todo/Cargo.toml` + `src/error.rs`
- `swissarmyhammer-issues/Cargo.toml` + `src/error.rs` (if error type exists)

## Estimated Changes

~120 lines of code (3 implementations + tests)

## Next Step

Step 8: Implement Severity for search/memoranda/outline errors

## Proposed Solution

After analyzing the existing error types in all three crates, here's my implementation plan:

### Analysis of Current Error Types

**GitError variants:**
- `RepositoryNotFound` - Error (operation failed)
- `RepositoryOperationFailed` - Error (generic operation failure)
- `BranchOperationFailed` - Error (branch operation failed)
- `BranchNotFound` - Error (missing resource)
- `BranchAlreadyExists` - Warning (conflict but not critical)
- `CommitOperationFailed` - Error (operation failed)
- `MergeOperationFailed` - Critical (merge conflicts need immediate attention)
- `WorkingDirectoryDirty` - Warning (informational state)
- `InvalidBranchName` - Error (invalid input)
- `Git2Error` - Error (propagated from library)
- `IoError` - Critical (filesystem issues)
- `Generic` - Error (unknown severity, default to Error)

**TodoError variants:**
- `Io` - Critical (filesystem failure)
- `Yaml` - Error (serialization issue)
- `Common` - Inherit from wrapped error's severity
- `InvalidTodoListName` - Error (invalid input)
- `InvalidTodoId` - Error (invalid input)
- `TodoListNotFound` - Error (resource not found)
- `TodoItemNotFound` - Error (resource not found)
- `EmptyTask` - Warning (validation issue)
- `Other` - Error (default)

**IssuesError (named Error) variants:**
- `IssueNotFound` - Error (resource not found)
- `IssueAlreadyExists` - Warning (conflict but can be handled)
- `Io` - Critical (filesystem failure)
- `Git` - Inherit from wrapped GitError's severity
- `Common` - Inherit from wrapped error's severity
- `Other` - Error (default)

### Implementation Steps

1. Implement `Severity` for `GitError` in `swissarmyhammer-git/src/error.rs`
2. Implement `Severity` for `TodoError` in `swissarmyhammer-todo/src/error.rs`
3. Implement `Severity` for `Error` in `swissarmyhammer-issues/src/error.rs`
4. Add comprehensive tests for each implementation
5. Build and test each crate individually
6. Run clippy on all three crates

### Testing Strategy

For each error type, I'll create tests that:
- Verify Critical severity for filesystem/corruption errors
- Verify Error severity for operation failures
- Verify Warning severity for informational issues
- Test that wrapped errors properly delegate severity (TodoError::Common, Error::Git, Error::Common)

## Implementation Notes

### GitError Implementation
- Added `use swissarmyhammer_common::{ErrorSeverity, Severity};` import
- Implemented `Severity` trait with the following mappings:
  - **Critical**: `IoError`, `MergeOperationFailed`
  - **Error**: `RepositoryNotFound`, `RepositoryOperationFailed`, `BranchOperationFailed`, `BranchNotFound`, `CommitOperationFailed`, `InvalidBranchName`, `Git2Error`, `Generic`
  - **Warning**: `BranchAlreadyExists`, `WorkingDirectoryDirty`
- Added comprehensive tests covering all three severity levels
- Tests pass: 17 tests run, all passed
- Clippy clean with no warnings

### TodoError Implementation
- Added `use swissarmyhammer_common::{ErrorSeverity, Severity};` import
- Implemented `Severity` trait with the following mappings:
  - **Critical**: `Io`
  - **Error**: `Yaml`, `InvalidTodoListName`, `InvalidTodoId`, `TodoListNotFound`, `TodoItemNotFound`, `Other`
  - **Warning**: `EmptyTask`
  - **Delegated**: `Common` (delegates to wrapped error's severity)
- Added tests including a test for `Common` error delegation
- Tests pass: 11 tests run, all passed
- Clippy clean with no warnings

### Issues Error Implementation
- Added `use swissarmyhammer_common::{ErrorSeverity, Severity};` import
- Implemented `Severity` trait for the `Error` type with the following mappings:
  - **Critical**: `Io`
  - **Error**: `IssueNotFound`, `Other`
  - **Warning**: `IssueAlreadyExists`
  - **Delegated**: `Git` (delegates to wrapped GitError's severity), `Common` (delegates to wrapped error's severity)
- Added tests including tests for both `Git` and `Common` error delegation
- Tests pass: 49 tests run, all passed
- Clippy clean with no warnings

### Key Design Decisions

1. **Delegation Pattern**: For wrapped errors (`TodoError::Common`, `Error::Git`, `Error::Common`), the implementation delegates to the wrapped error's `severity()` method. This ensures consistent severity reporting across error boundaries.

2. **Filesystem Errors are Critical**: All `Io` errors are marked as `Critical` because filesystem failures prevent the system from functioning properly and require immediate attention.

3. **Merge Conflicts are Critical**: Git merge conflicts are marked as `Critical` because they require immediate user intervention to resolve.

4. **Validation Errors are Warnings**: Issues like `EmptyTask` and `IssueAlreadyExists` are marked as `Warning` because they can be handled without system failure.

5. **Missing Resources are Errors**: Not found errors (branches, issues, todos) are `Error` severity because the operation failed but the system remains functional.

### Files Modified
- `swissarmyhammer-git/src/error.rs` - Added Severity impl and tests
- `swissarmyhammer-todo/src/error.rs` - Added Severity impl and tests
- `swissarmyhammer-issues/src/error.rs` - Added Severity impl and tests

### All Acceptance Criteria Met
- ✅ GitError implements Severity trait
- ✅ TodoError implements Severity trait
- ✅ IssuesError (Error type) implements Severity trait
- ✅ Unit tests for each implementation
- ✅ Tests pass for all three crates (77 tests total, all passed)
- ✅ Code compiles for all three crates
- ✅ Clippy clean for all three crates
