# Step 12: Implement Severity for PlanCommandError

**Refer to ideas/severity.md**

## Goal

Refactor the existing `severity()` method on `PlanCommandError` to implement the `Severity` trait.

## Context

PlanCommandError in the main swissarmyhammer crate ALREADY has a severity() method (lines 113-135 of `swissarmyhammer/src/plan_utils.rs`). We need to refactor it to implement the trait.

This is exactly like what was done for ValidationError in step 4.

## Proposed Solution

I will refactor the existing `severity()` method on `PlanCommandError` to implement the `Severity` trait by:

1. **Adding the import**: Add `use swissarmyhammer_common::Severity;` at the top of the file
2. **Converting the method to trait implementation**: Change from `impl PlanCommandError { pub fn severity(...) }` to `impl Severity for PlanCommandError { fn severity(...) }`
3. **Preserving all existing severity assignments** - the match logic remains identical
4. **Writing tests first** (TDD approach):
   - Test Critical severity cases (PermissionDenied, WorkflowExecutionFailed, IssuesDirectoryNotWritable)
   - Test Error severity cases (FileNotFound, InvalidFileFormat, EmptyPlanFile, IssueCreationFailed)
   - Test Warning severity cases (FileTooLarge, InsufficientContent)
5. **Running tests to verify** they pass after the refactor
6. **Verifying the build** with cargo build and cargo clippy

The refactor is straightforward because:
- The method signature stays the same
- The implementation logic is unchanged
- Only the trait implementation syntax changes
- No existing callers need modification

## Implementation Notes

### Changes Made

1. **Added Severity trait import** at swissarmyhammer/src/plan_utils.rs:7
   - Changed: `use swissarmyhammer_common::SwissArmyHammerError;`
   - To: `use swissarmyhammer_common::{Severity, SwissArmyHammerError};`

2. **Refactored severity() method to trait implementation** at swissarmyhammer/src/plan_utils.rs:103-135
   - Moved `severity()` method from `impl PlanCommandError` block
   - Created new `impl Severity for PlanCommandError` block
   - Preserved all severity assignments exactly as they were:
     - **Critical**: PermissionDenied, WorkflowExecutionFailed, IssuesDirectoryNotWritable
     - **Error**: FileNotFound, InvalidFileFormat, EmptyPlanFile, IssueCreationFailed
     - **Warning**: FileTooLarge, InsufficientContent

3. **Added comprehensive tests** at swissarmyhammer/src/plan_utils.rs:619-685
   - `test_plan_command_error_severity_critical()` - Tests all 3 Critical severity variants
   - `test_plan_command_error_severity_error()` - Tests all 4 Error severity variants
   - `test_plan_command_error_severity_warning()` - Tests all 2 Warning severity variants
   - All tests import and use the Severity trait to verify trait implementation

### Verification

- ✅ All 134 tests in swissarmyhammer package pass
- ✅ cargo build -p swissarmyhammer succeeds
- ✅ cargo clippy -p swissarmyhammer is clean (no warnings or errors)
- ✅ All severity assignments preserved exactly as before
- ✅ Trait implementation is consistent with previous steps (e.g., ValidationError in step 4)

### Files Modified

- `swissarmyhammer/src/plan_utils.rs`
  - Added Severity trait import
  - Refactored severity() method to trait implementation
  - Added 3 new test functions with 9 test cases total

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

- ✅ PlanCommandError implements Severity trait
- ✅ All existing severity assignments preserved
- ✅ Use statement added for Severity trait
- ✅ Tests added or existing tests pass
- ✅ Tests pass: `cargo test -p swissarmyhammer`
- ✅ Code compiles: `cargo build -p swissarmyhammer`
- ✅ Clippy clean: `cargo clippy -p swissarmyhammer`

## Files to Modify

- `swissarmyhammer/src/plan_utils.rs` (refactor method to trait impl + tests)

## Estimated Changes

~30 lines of code (refactor + tests)

## Next Step

Step 13: Integration testing and documentation
