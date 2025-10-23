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



## Proposed Solution

After analyzing the codebase, I've identified the error types and their appropriate severity mappings:

### ShellSecurityError Analysis
Located in `swissarmyhammer-shell/src/security.rs`, this enum has these variants:
- `BlockedCommandPattern` - Critical (security violation)
- `CommandTooLong` - Error (policy violation but not security-critical)
- `DirectoryAccessDenied` - Critical (unauthorized access attempt)
- `InvalidDirectory` - Error (configuration/validation issue)
- `InvalidEnvironmentVariable` - Error (validation issue)
- `InvalidEnvironmentVariableValue` - Error (validation issue)
- `ValidationFailed` - Error (general validation failure)

### ShellError Analysis
Located in `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`, this enum has these variants:
- `CommandSpawnError` - Critical (system cannot execute commands)
- `ExecutionError` - Error (command failed but system functional)
- `InvalidCommand` - Error (user input validation)
- `SystemError` - Critical (system-level failure)
- `WorkingDirectoryError` - Error (configuration issue)

### Implementation Plan

1. **Add Severity import to security.rs**
   - Import `ErrorSeverity` and `Severity` from `swissarmyhammer-common`

2. **Implement Severity for ShellSecurityError**
   - All security violations (BlockedCommandPattern, DirectoryAccessDenied) → Critical
   - Validation failures → Error

3. **Add Severity import to shell/execute/mod.rs**
   - Import `ErrorSeverity` and `Severity` from `swissarmyhammer-common`

4. **Implement Severity for ShellError**
   - System failures (CommandSpawnError, SystemError) → Critical
   - Execution/validation failures → Error

5. **Add comprehensive tests**
   - Test each error variant returns correct severity
   - Ensure all security errors are Critical

### Rationale

**ShellSecurityError severity decisions:**
- `BlockedCommandPattern`: **Critical** - Attempted execution of dangerous commands
- `DirectoryAccessDenied`: **Critical** - Unauthorized access attempt (path traversal, etc.)
- `CommandTooLong`: **Error** - Policy violation but system remains functional
- `InvalidDirectory`, `InvalidEnvironmentVariable*`, `ValidationFailed`: **Error** - Validation issues

**ShellError severity decisions:**
- `CommandSpawnError`: **Critical** - Shell system cannot spawn processes (broken)
- `SystemError`: **Critical** - System-level failure affects shell functionality
- `ExecutionError`: **Error** - Command failed but shell system functional
- `InvalidCommand`: **Error** - User input issue, not system failure
- `WorkingDirectoryError`: **Error** - Configuration issue, not critical system failure




## Implementation Notes

### Files Modified

1. **swissarmyhammer-shell/src/security.rs**
   - Added import: `use swissarmyhammer_common::{ErrorSeverity, Result, Severity, SwissArmyHammerError};`
   - Implemented `Severity` trait for `ShellSecurityError` at line 83-98
   - Added three comprehensive test functions:
     - `test_shell_security_error_severity_critical()` - Tests security violations
     - `test_shell_security_error_severity_error()` - Tests validation failures
     - `test_all_security_violations_are_critical()` - Ensures security errors are Critical

2. **swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs**
   - Added import: `use swissarmyhammer_common::{ErrorSeverity, Severity};`
   - Implemented `Severity` trait for `ShellError` at line 646-659
   - Added three comprehensive test functions:
     - `test_shell_error_severity_critical()` - Tests system-level failures
     - `test_shell_error_severity_error()` - Tests execution/validation failures
     - `test_all_shell_errors_have_severity()` - Ensures all variants covered

### Implementation Details

**ShellSecurityError severity mapping:**
- **Critical** (2 variants): `BlockedCommandPattern`, `DirectoryAccessDenied` - Actual security violations
- **Error** (5 variants): `CommandTooLong`, `InvalidDirectory`, `InvalidEnvironmentVariable`, `InvalidEnvironmentVariableValue`, `ValidationFailed` - Validation failures

**ShellError severity mapping:**
- **Critical** (2 variants): `CommandSpawnError`, `SystemError` - System cannot function
- **Error** (3 variants): `ExecutionError`, `InvalidCommand`, `WorkingDirectoryError` - Operation failed but system functional

### Test Results

All tests pass successfully:
- `cargo nextest run -p swissarmyhammer-shell`: 21 tests passed
- `cargo nextest run -p swissarmyhammer-tools -E 'test(shell)'`: 77 tests passed
- `cargo clippy -p swissarmyhammer-shell -- -D warnings`: No warnings
- `cargo clippy -p swissarmyhammer-tools --tests -- -D warnings`: No warnings

### Verification

✅ ShellSecurityError implements Severity trait
✅ ShellError implements Severity trait
✅ Unit tests for both implementations
✅ All security errors return Critical severity
✅ Tests pass: `cargo test -p swissarmyhammer-shell`
✅ Code compiles: `cargo build -p swissarmyhammer-shell`
✅ Clippy clean: `cargo clippy -p swissarmyhammer-shell`

All acceptance criteria met. Implementation complete and tested.

