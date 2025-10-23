# Step 9: Implement Severity for Templating and Agent-Executor Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for TemplatingError and ActionError (in agent-executor).

## Context

The templating crate handles Liquid template rendering, and agent-executor handles LLM agent action execution. Both have errors ranging from template syntax errors to agent execution failures.

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure both crates depend on swissarmyhammer-common:

```toml
# In swissarmyhammer-templating/Cargo.toml
# In swissarmyhammer-agent-executor/Cargo.toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for TemplatingError

In `swissarmyhammer-templating/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for TemplatingError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Template system cannot function
            TemplatingError::TemplateEngineInitFailed { .. } => ErrorSeverity::Critical,
            TemplatingError::InvalidTemplateSyntax { .. } => ErrorSeverity::Critical,
            
            // Error: Template rendering failed
            TemplatingError::RenderingFailed { .. } => ErrorSeverity::Error,
            TemplatingError::TemplateNotFound { .. } => ErrorSeverity::Error,
            TemplatingError::MissingVariable { .. } => ErrorSeverity::Error,
            TemplatingError::InvalidVariableType { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            TemplatingError::UnusedVariable { .. } => ErrorSeverity::Warning,
            TemplatingError::DeprecatedSyntax { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 3. Implement Severity for ActionError (agent-executor)

In `swissarmyhammer-agent-executor/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ActionError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Agent system cannot function
            ActionError::AgentInitializationFailed { .. } => ErrorSeverity::Critical,
            ActionError::ModelUnavailable { .. } => ErrorSeverity::Critical,
            ActionError::SystemError { .. } => ErrorSeverity::Critical,
            
            // Error: Action execution failed
            ActionError::ExecutionFailed { .. } => ErrorSeverity::Error,
            ActionError::InvalidAction { .. } => ErrorSeverity::Error,
            ActionError::TimeoutError { .. } => ErrorSeverity::Error,
            ActionError::InvalidInput { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical issues
            ActionError::PerformanceDegradation { .. } => ErrorSeverity::Warning,
            ActionError::PartialSuccess { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 4. Add Tests for Both Implementations

In `swissarmyhammer-templating/src/error.rs`:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_templating_error_critical() {
        let error = TemplatingError::TemplateEngineInitFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_templating_error_error_level() {
        let error = TemplatingError::RenderingFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_templating_error_warning() {
        let error = TemplatingError::UnusedVariable { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
```

In `swissarmyhammer-agent-executor/src/error.rs`:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_action_error_critical() {
        let error = ActionError::AgentInitializationFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_action_error_error_level() {
        let error = ActionError::ExecutionFailed { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_action_error_warning() {
        let error = ActionError::PerformanceDegradation { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
```

## Severity Guidelines

### Templating Errors
**Critical**: Template engine initialization failed, invalid syntax
**Error**: Rendering failures, template not found, variable issues
**Warning**: Unused variables, deprecated syntax

### Agent-Executor Errors
**Critical**: Agent initialization failed, model unavailable, system errors
**Error**: Execution failures, invalid actions, timeouts
**Warning**: Performance issues, partial success

## Acceptance Criteria

- [ ] TemplatingError implements Severity trait
- [ ] ActionError implements Severity trait
- [ ] Unit tests for both implementations
- [ ] Tests pass for both crates
- [ ] Code compiles for both crates
- [ ] Clippy clean for both crates

## Files to Modify

- `swissarmyhammer-templating/Cargo.toml` + `src/error.rs`
- `swissarmyhammer-agent-executor/Cargo.toml` + `src/error.rs`

## Estimated Changes

~80 lines of code (2 implementations + tests)

## Next Step

Step 10: Implement Severity for shell errors



## Proposed Solution

After examining the actual error types in both crates, I will implement the `Severity` trait for:

1. **TemplatingError** (in swissarmyhammer-templating/src/error.rs):
   - Parse, Render, Security, Partial, VariableExtraction errors → Error
   - Timeout → Error (operation failed)
   - Io, Json → Error (propagated errors)
   - Other → Error (generic failures)

2. **ActionError** (in swissarmyhammer-agent-executor/src/error.rs):
   - ClaudeError → Critical (model/API unavailable prevents agent operation)
   - VariableError, ParseError, ExecutionError → Error (specific operation failures)
   - IoError, JsonError → Error (propagated errors)
   - RateLimit → Warning (temporary issue, can retry)

### Implementation Steps:
1. Add swissarmyhammer-common dependency to agent-executor (templating already has it)
2. Write failing tests for TemplatingError severity
3. Implement Severity trait for TemplatingError
4. Write failing tests for ActionError severity
5. Implement Severity trait for ActionError
6. Run tests and ensure they pass
7. Run clippy and cargo build

### Rationale:
- **Critical**: Reserved for system-level failures where the agent/template engine cannot function at all
- **Error**: Operation-specific failures that prevent completing a task but system remains stable
- **Warning**: Temporary or non-blocking issues (like rate limits where retry is possible)



## Implementation Notes

Successfully implemented the `Severity` trait for both TemplatingError and ActionError following Test-Driven Development principles.

### TemplatingError (swissarmyhammer-templating/src/error.rs)

All error variants categorized as **Error** severity level:
- Parse, Render, Security, Partial, VariableExtraction errors prevent successful template operations
- Timeout, Io, Json, Other errors are operation-level failures
- System remains stable but specific template operation cannot complete

### ActionError (swissarmyhammer-agent-executor/src/error.rs)

Error variants categorized by severity:
- **Critical**: ClaudeError (model/API unavailable prevents agent execution)
- **Error**: VariableError, ParseError, ExecutionError, IoError, JsonError (operation-specific failures)
- **Warning**: RateLimit (temporary issue with retry timing, can be retried)

### Testing

Created comprehensive unit tests for both implementations:
- **TemplatingError**: 9 tests covering all error variants
- **ActionError**: 7 tests covering all error variants
- All tests passing

### Code Quality

- Cargo build: ✅ Clean compilation
- Cargo clippy: ✅ No warnings
- Cargo nextest: ✅ All tests pass
- Added comprehensive documentation for both implementations

### Files Modified

1. swissarmyhammer-agent-executor/Cargo.toml:3:22 - Added swissarmyhammer-common dependency
2. swissarmyhammer-templating/src/error.rs:3 - Added Severity import and implementation with tests
3. swissarmyhammer-agent-executor/src/error.rs:49 - Added Severity import and implementation with tests

Total changes: ~140 lines of code (2 implementations + comprehensive tests + documentation)
