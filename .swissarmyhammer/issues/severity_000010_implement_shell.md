# Step 10: Implement Severity for Shell Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for ShellSecurityError and ShellError in the swissarmyhammer-shell crate.

## Context

The shell crate handles command execution with security checks. Security violations are critical, while execution failures are errors.

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure `swissarmyhammer-shell/Cargo.toml` depends on swissarmyhammer-common:

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for ShellSecurityError

In `swissarmyhammer-shell/src/security.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ShellSecurityError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // All security violations are Critical
            ShellSecurityError::UnsafeCommand { .. } => ErrorSeverity::Critical,
            ShellSecurityError::PathTraversal { .. } => ErrorSeverity::Critical,
            ShellSecurityError::UnauthorizedAccess { .. } => ErrorSeverity::Critical,
            ShellSecurityError::CommandInjection { .. } => ErrorSeverity::Critical,
            ShellSecurityError::PrivilegeEscalation { .. } => ErrorSeverity::Critical,
        }
    }
}
```

### 3. Implement Severity for ShellError

In `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` (or wherever ShellError is defined):

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ShellError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Shell system cannot function
            ShellError::SystemFailure { .. } => ErrorSeverity::Critical,
            ShellError::SecurityViolation { .. } => ErrorSeverity::Critical,
            
            // Error: Command execution failed
            ShellError::ExecutionFailed { .. } => ErrorSeverity::Error,
            ShellError::CommandNotFound { .. } => ErrorSeverity::Error,
            ShellError::TimeoutExceeded { .. } => ErrorSeverity::Error,
            ShellError::InvalidCommand { .. } => ErrorSeverity::Error,
            ShellError::NonZeroExit { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            ShellError::OutputTruncated { .. } => ErrorSeverity::Warning,
            ShellError::PerformanceDegradation { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 4. Add Tests

In `swissarmyhammer-shell/src/security.rs`:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_all_security_errors_are_critical() {
        let errors = vec![
            ShellSecurityError::UnsafeCommand { /* fields */ },
            ShellSecurityError::PathTraversal { /* fields */ },
            ShellSecurityError::UnauthorizedAccess { /* fields */ },
            ShellSecurityError::CommandInjection { /* fields */ },
        ];
        
        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Critical,
                "All security errors must be Critical: {}",
                error
            );
        }
    }
}
```

In the ShellError file:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_shell_error_critical() {
        let error = ShellError::SystemFailure { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_shell_error_error_level() {
        let error = ShellError::ExecutionFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_shell_error_warning() {
        let error = ShellError::OutputTruncated { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
```

## Severity Guidelines

### Security Errors
**Critical**: ALL security violations (by definition)
- Unsafe commands
- Path traversal attempts
- Unauthorized access
- Command injection
- Privilege escalation

### Shell Errors
**Critical**: System failures, security violations
**Error**: Command execution failures, timeouts, non-zero exits
**Warning**: Output truncation, performance issues

## Acceptance Criteria

- [ ] ShellSecurityError implements Severity trait
- [ ] ShellError implements Severity trait
- [ ] Unit tests for both implementations
- [ ] All security errors return Critical severity
- [ ] Tests pass: `cargo test -p swissarmyhammer-shell`
- [ ] Code compiles: `cargo build -p swissarmyhammer-shell`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-shell`

## Files to Modify

- `swissarmyhammer-shell/Cargo.toml` (add dependency if needed)
- `swissarmyhammer-shell/src/security.rs` (implementation + tests)
- Location of ShellError definition (implementation + tests)

## Estimated Changes

~80 lines of code (2 implementations + tests)

## Next Step

Step 11: Implement Severity for MCP tool errors
