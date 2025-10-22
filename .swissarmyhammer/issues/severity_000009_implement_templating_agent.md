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
