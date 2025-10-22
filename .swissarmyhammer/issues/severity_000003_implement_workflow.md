# Step 3: Implement Severity for Workflow Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for all workflow-related error types in swissarmyhammer-workflow crate.

## Context

The workflow crate contains several error types that need severity implementations:
- WorkflowError
- ExecutorError  
- GraphError
- StateError
- ActionError
- ParseError

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure `swissarmyhammer-workflow/Cargo.toml` depends on swissarmyhammer-common:

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for WorkflowError

In `swissarmyhammer-workflow/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for WorkflowError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            WorkflowError::InvalidState { .. } => ErrorSeverity::Critical,
            WorkflowError::TransitionFailed { .. } => ErrorSeverity::Critical,
            // Add other variants with appropriate severity
            _ => ErrorSeverity::Error,
        }
    }
}
```

### 3. Implement Severity for ExecutorError

In `swissarmyhammer-workflow/src/executor/mod.rs`:

```rust
impl Severity for ExecutorError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ExecutorError::WorkflowFailed { .. } => ErrorSeverity::Critical,
            ExecutorError::ActionFailed { .. } => ErrorSeverity::Error,
            // Add other variants
            _ => ErrorSeverity::Error,
        }
    }
}
```

### 4. Implement Severity for Other Error Types

Implement for:
- `GraphError` in `swissarmyhammer-workflow/src/graph.rs`
- `StateError` in `swissarmyhammer-workflow/src/state.rs`
- `ActionError` in `swissarmyhammer-workflow/src/actions.rs`
- `ParseError` in `swissarmyhammer-workflow/src/parser.rs`

### 5. Add Tests

For each implementation, add unit tests verifying severity assignments:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_workflow_error_severity() {
        let error = WorkflowError::InvalidState { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        // Test other variants
    }
}
```

## Severity Guidelines for Workflow Errors

**Critical**:
- Workflow execution failures
- Invalid state transitions
- Data corruption
- Unable to recover workflow state

**Error**:
- Action failures (can retry)
- Invalid configurations
- Missing resources

**Warning**:
- Deprecated workflow features
- Performance issues
- Optional features unavailable

## Acceptance Criteria

- [ ] All 6 workflow error types implement Severity
- [ ] Each error variant has appropriate severity
- [ ] Unit tests for each implementation
- [ ] Tests pass: `cargo test -p swissarmyhammer-workflow`
- [ ] Code compiles: `cargo build -p swissarmyhammer-workflow`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-workflow`

## Files to Modify

- `swissarmyhammer-workflow/Cargo.toml` (add dependency)
- `swissarmyhammer-workflow/src/error.rs`
- `swissarmyhammer-workflow/src/executor/mod.rs`
- `swissarmyhammer-workflow/src/graph.rs`
- `swissarmyhammer-workflow/src/state.rs`
- `swissarmyhammer-workflow/src/actions.rs`
- `swissarmyhammer-workflow/src/parser.rs`

## Estimated Changes

~150 lines of code (6 implementations + tests)

## Next Step

Step 4: Implement Severity for CLI errors
