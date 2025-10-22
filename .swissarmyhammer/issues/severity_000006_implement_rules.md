# Step 6: Implement Severity for Rules Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for RuleError in swissarmyhammer-rules crate.

## Context

The rules crate handles code quality and standards checking. Rule violations can range from style issues (warnings) to critical problems that block builds.

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure `swissarmyhammer-rules/Cargo.toml` depends on swissarmyhammer-common:

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for RuleError

In `swissarmyhammer-rules/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for RuleError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Rule system cannot function
            RuleError::InvalidRuleDefinition { .. } => ErrorSeverity::Critical,
            RuleError::RuleLoadingFailed { .. } => ErrorSeverity::Critical,
            RuleError::CompilationFailed { .. } => ErrorSeverity::Critical,
            
            // Error: Rule check failed
            RuleError::RuleViolation { .. } => ErrorSeverity::Error,
            RuleError::FileAccessError { .. } => ErrorSeverity::Error,
            RuleError::ParseError { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical rule issues
            RuleError::DeprecatedRule { .. } => ErrorSeverity::Warning,
            RuleError::PerformanceDegradation { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 3. Add Tests

Create comprehensive tests:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_critical_rule_errors() {
        let errors = vec![
            RuleError::InvalidRuleDefinition { /* fields */ },
            RuleError::RuleLoadingFailed { /* fields */ },
            RuleError::CompilationFailed { /* fields */ },
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
    fn test_error_level_rule_errors() {
        let errors = vec![
            RuleError::RuleViolation { /* fields */ },
            RuleError::FileAccessError { /* fields */ },
        ];
        
        for error in errors {
            assert_eq!(error.severity(), ErrorSeverity::Error);
        }
    }

    #[test]
    fn test_warning_rule_errors() {
        let error = RuleError::DeprecatedRule { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
```

## Severity Guidelines for Rule Errors

**Critical**:
- Invalid rule definitions (rule system cannot work)
- Rule loading failures
- Compilation failures that prevent checking

**Error**:
- Rule violations found in code
- File access errors during checking
- Parse errors in code being checked

**Warning**:
- Deprecated rules in use
- Performance degradation during checking
- Optional checks that failed

## Acceptance Criteria

- [ ] RuleError implements Severity trait
- [ ] All error variants have appropriate severity
- [ ] Comprehensive unit tests
- [ ] Tests pass: `cargo test -p swissarmyhammer-rules`
- [ ] Code compiles: `cargo build -p swissarmyhammer-rules`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-rules`

## Files to Modify

- `swissarmyhammer-rules/Cargo.toml` (add dependency if needed)
- `swissarmyhammer-rules/src/error.rs` (implementation + tests)

## Estimated Changes

~60 lines of code (implementation + tests)

## Next Step

Step 7: Implement Severity for git/issues/todo errors
