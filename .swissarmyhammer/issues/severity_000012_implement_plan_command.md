# Step 12: Implement Severity for PlanCommandError

**Refer to ideas/severity.md**

## Goal

Refactor the existing `severity()` method on `PlanCommandError` to implement the `Severity` trait.

## Context

PlanCommandError in the main swissarmyhammer crate ALREADY has a severity() method (lines 113-135 of `swissarmyhammer/src/plan_utils.rs`). We need to refactor it to implement the trait.

This is exactly like what we did for ValidationError in step 4.

## Tasks

### 1. Ensure swissarmyhammer-common Dependency

Verify `swissarmyhammer/Cargo.toml` depends on swissarmyhammer-common (it already does based on the existing code).

### 2. Refactor Existing severity() Method to Trait Implementation

In `swissarmyhammer/src/plan_utils.rs`:

**Before** (lines 113-135):
```rust
pub fn severity(&self) -> swissarmyhammer_common::ErrorSeverity {
    match self {
        PlanCommandError::FileNotFound { .. } => swissarmyhammer_common::ErrorSeverity::Error,
        // ... other variants
    }
}
```

**After**:
```rust
use swissarmyhammer_common::Severity;

impl Severity for PlanCommandError {
    fn severity(&self) -> swissarmyhammer_common::ErrorSeverity {
        match self {
            PlanCommandError::FileNotFound { .. } => swissarmyhammer_common::ErrorSeverity::Error,
            PlanCommandError::PermissionDenied { .. } => {
                swissarmyhammer_common::ErrorSeverity::Critical
            }
            PlanCommandError::InvalidFileFormat { .. } => {
                swissarmyhammer_common::ErrorSeverity::Error
            }
            PlanCommandError::WorkflowExecutionFailed { .. } => {
                swissarmyhammer_common::ErrorSeverity::Critical
            }
            PlanCommandError::FileTooLarge { .. } => swissarmyhammer_common::ErrorSeverity::Warning,
            PlanCommandError::EmptyPlanFile { .. } => swissarmyhammer_common::ErrorSeverity::Error,
            PlanCommandError::IssueCreationFailed { .. } => {
                swissarmyhammer_common::ErrorSeverity::Error
            }
            PlanCommandError::IssuesDirectoryNotWritable { .. } => {
                swissarmyhammer_common::ErrorSeverity::Critical
            }
            PlanCommandError::InsufficientContent { .. } => {
                swissarmyhammer_common::ErrorSeverity::Warning
            }
        }
    }
}
```

### 3. Add Use Statement

Ensure the Severity trait is imported at the top of the file:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};
```

### 4. Verify Existing Usage

Check if any code calls `error.severity()` on PlanCommandError. The method signature hasn't changed, so existing code should continue to work.

### 5. Add/Update Tests

If tests exist for the severity method, verify they still pass. If not, add tests:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_plan_command_error_critical() {
        let error = PlanCommandError::PermissionDenied { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        let error = PlanCommandError::WorkflowExecutionFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_plan_command_error_error_level() {
        let error = PlanCommandError::FileNotFound { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
        
        let error = PlanCommandError::InvalidFileFormat { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_plan_command_error_warning() {
        let error = PlanCommandError::FileTooLarge { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
        
        let error = PlanCommandError::InsufficientContent { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
```

## Severity Assignments (Already Correct)

**Critical**:
- PermissionDenied
- WorkflowExecutionFailed
- IssuesDirectoryNotWritable

**Error**:
- FileNotFound
- InvalidFileFormat
- EmptyPlanFile
- IssueCreationFailed

**Warning**:
- FileTooLarge
- InsufficientContent

## Acceptance Criteria

- [ ] PlanCommandError implements Severity trait
- [ ] All existing severity assignments preserved
- [ ] Use statement added for Severity trait
- [ ] Tests added or existing tests pass
- [ ] Tests pass: `cargo test -p swissarmyhammer`
- [ ] Code compiles: `cargo build -p swissarmyhammer`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer`

## Files to Modify

- `swissarmyhammer/src/plan_utils.rs` (refactor method to trait impl + tests)

## Estimated Changes

~30 lines of code (refactor + tests)

## Next Step

Step 13: Integration testing and documentation
