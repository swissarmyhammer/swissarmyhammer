# Step 2: Implement Severity for SwissArmyHammerError

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for `SwissArmyHammerError` in swissarmyhammer-common, serving as the reference implementation for all other error types.

## Context

SwissArmyHammerError is the main error type in swissarmyhammer-common and is used throughout the codebase. This implementation will serve as a template for implementing the trait in other crates.

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
