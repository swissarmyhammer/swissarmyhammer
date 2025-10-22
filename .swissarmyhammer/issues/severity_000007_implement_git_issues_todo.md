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
