# Step 2: Implement Severity for SwissArmyHammerError

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for `SwissArmyHammerError` in swissarmyhammer-common, serving as the reference implementation for all other error types.

## Context

SwissArmyHammerError is the main error type in swissarmyhammer-common and is used throughout the codebase. This implementation will serve as a template for implementing the trait in other crates.

## Proposed Solution

I will follow Test-Driven Development to implement the `Severity` trait for `SwissArmyHammerError`:

### Phase 1: Write Failing Tests
1. Create test cases for all three severity levels (Warning, Error, Critical)
2. Tests will verify each error variant returns the expected severity
3. Run tests to confirm they fail (trait not yet implemented)

### Phase 2: Implement the Trait
1. Add `impl Severity for SwissArmyHammerError` with a match expression covering all variants
2. Assign severity levels following these principles:
   - **Critical**: Data loss, system cannot continue (NotInGitRepository, DirectoryCreation, WorkflowNotFound, Storage, PermissionDenied)
   - **Error**: Operation failed but recoverable (Io, Serialization, Json, FileNotFound, NotAFile, InvalidFilePath, InvalidPath, IoContext, Semantic, Context, Other)
   - **Warning**: Non-critical issues (RuleViolation)
3. Add documentation explaining severity decisions

### Phase 3: Verify
1. Run tests to confirm they pass
2. Run `cargo build -p swissarmyhammer-common` to ensure compilation
3. Run `cargo clippy -p swissarmyhammer-common` for linting
4. Format with `cargo fmt`

### Severity Mapping Rationale
- **Critical errors**: Prevent system from functioning normally - repository issues, directory access, workflow execution, storage failures
- **Error level**: Operation-specific failures that don't prevent the system from functioning - file operations, format issues, I/O errors
- **Warning level**: Informational issues that don't prevent operation - rule violations

This approach ensures comprehensive test coverage before implementation and validates the solution works correctly.

## Tasks

### 1. Implement Severity for SwissArmyHammerError

Add implementation in `swissarmyhammer-common/src/error.rs`:

```rust
impl Severity for SwissArmyHammerError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Data loss or system cannot continue
            SwissArmyHammerError::NotInGitRepository => ErrorSeverity::Critical,
            SwissArmyHammerError::DirectoryCreation(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::DirectoryAccess(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::WorkflowNotFound(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::WorkflowRunNotFound(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::Storage(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::PermissionDenied { .. } => ErrorSeverity::Critical,
            
            // Error: Operation failed but recoverable
            SwissArmyHammerError::Io(_) => ErrorSeverity::Error,
            SwissArmyHammerError::Serialization(_) => ErrorSeverity::Error,
            SwissArmyHammerError::Json(_) => ErrorSeverity::Error,
            SwissArmyHammerError::FileNotFound { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::NotAFile { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::InvalidFilePath { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::InvalidPath { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::IoContext { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::Semantic { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::Context { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::Other { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            SwissArmyHammerError::RuleViolation(_) => ErrorSeverity::Warning,
        }
    }
}
```

### 2. Add Tests

Create comprehensive tests in `swissarmyhammer-common/src/error.rs`:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn test_critical_errors() {
        let errors = vec![
            SwissArmyHammerError::NotInGitRepository,
            SwissArmyHammerError::DirectoryCreation("test".to_string()),
            SwissArmyHammerError::WorkflowNotFound("test".to_string()),
        ];
        
        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Critical,
                "Expected Critical severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_error_level_errors() {
        let errors = vec![
            SwissArmyHammerError::FileNotFound {
                path: "test".to_string(),
                suggestion: "check path".to_string(),
            },
            SwissArmyHammerError::InvalidFilePath {
                path: "test".to_string(),
                suggestion: "fix path".to_string(),
            },
        ];
        
        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Error,
                "Expected Error severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_warning_errors() {
        let error = SwissArmyHammerError::RuleViolation("test".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
```

### 3. Document Severity Decisions

Add comments explaining the severity assignments above the impl block.

## Acceptance Criteria

- [ ] Severity trait implemented for SwissArmyHammerError
- [ ] All error variants have appropriate severity levels
- [ ] Comprehensive unit tests for all three severity levels
- [ ] Tests pass: `cargo test -p swissarmyhammer-common`
- [ ] Code compiles: `cargo build -p swissarmyhammer-common`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-common`

## Files to Modify

- `swissarmyhammer-common/src/error.rs` (implementation + tests)

## Estimated Changes

~80 lines of code (implementation + tests)

## Next Step

Step 3: Implement Severity for workflow errors


## Implementation Notes

### Completed: 2025-10-22

Successfully implemented the `Severity` trait for `SwissArmyHammerError` following Test-Driven Development methodology.

#### Implementation Steps Taken

1. **Test Creation** (Lines 471-549 in error.rs)
   - Created `test_swissarmyhammer_error_critical_severity()` - Tests 7 critical error variants
   - Created `test_swissarmyhammer_error_error_severity()` - Tests 8 error-level variants
   - Created `test_swissarmyhammer_error_warning_severity()` - Tests 1 warning variant
   - All tests initially failed as expected (trait not implemented)

2. **Trait Implementation** (Lines 338-390 in error.rs)
   - Added comprehensive documentation explaining severity assignment guidelines
   - Implemented `Severity` trait for `SwissArmyHammerError`
   - Used exhaustive match to cover all error variants
   - Added inline comments categorizing errors by severity level

3. **Severity Assignments Made**

   **Critical (7 variants)** - System-level failures:
   - `NotInGitRepository` - Cannot operate without git repository
   - `DirectoryCreation` - Critical directory setup failed
   - `DirectoryAccess` - Cannot access required directories
   - `WorkflowNotFound` - Core workflow system failure
   - `WorkflowRunNotFound` - Workflow execution system failure
   - `Storage` - Storage backend failure
   - `PermissionDenied` - Critical resource access denied

   **Error (11 variants)** - Operation-specific failures:
   - `Io` - General I/O errors
   - `Serialization` - YAML serialization errors
   - `Json` - JSON serialization errors
   - `FileNotFound` - File operation failed
   - `NotAFile` - Path type mismatch
   - `InvalidFilePath` - Invalid file path format
   - `InvalidPath` - Invalid path encountered
   - `IoContext` - Contextual I/O error
   - `Semantic` - Semantic search error
   - `Context` - Generic error with context
   - `Other` - Custom error messages

   **Warning (1 variant)** - Non-critical issues:
   - `RuleViolation` - Code quality issue, non-blocking

#### Verification Results

✅ All tests pass (199/199):
```
cargo nextest run --package swissarmyhammer-common
Summary [0.273s] 199 tests run: 199 passed, 0 skipped
```

✅ Build successful:
```
cargo build -p swissarmyhammer-common
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s
```

✅ Clippy clean:
```
cargo clippy -p swissarmyhammer-common
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.69s
```

✅ Code formatted:
```
cargo fmt --all
```

#### Design Decisions

1. **Critical vs Error distinction**
   - Critical errors prevent the system from functioning (repository setup, workflow system)
   - Error-level failures are operation-specific but system can continue
   - This follows the guidelines in ErrorSeverity documentation

2. **PermissionDenied as Critical**
   - Marked as Critical because permission issues typically indicate system configuration problems
   - Prevents continued operation until resolved

3. **RuleViolation as Warning**
   - Non-blocking code quality issues
   - System can continue despite violations
   - Consistent with existing `PlanCommandError` implementation

4. **Documentation**
   - Added comprehensive doc comments explaining severity assignments
   - Included examples for each severity level
   - Provided clear guidelines for future implementations

#### Files Modified

- `/Users/wballard/github/swissarmyhammer/swissarmyhammer-common/src/error.rs`
  - Added Severity trait implementation (lines 338-390)
  - Added comprehensive tests (lines 471-549)
  - Total: ~108 lines added

#### Impact

This implementation serves as the reference for all other error types in the codebase. Future implementations should follow this pattern:
- Comprehensive test coverage for all severity levels
- Clear documentation of severity assignment rationale
- Exhaustive match expressions covering all variants

#### Next Steps

Ready to proceed with Step 3: Implement Severity for workflow errors in swissarmyhammer-workflow crate.
