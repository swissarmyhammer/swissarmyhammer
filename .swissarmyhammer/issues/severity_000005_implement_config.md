# Step 5: Implement Severity for Config Errors

**Refer to ideas/severity.md**

## Goal

Implement the `Severity` trait for configuration error types: ConfigurationError and AgentError.

## Context

The swissarmyhammer-config crate handles configuration parsing and validation. Errors here range from missing config files (warnings) to invalid schema (critical).

## Tasks

### 1. Add swissarmyhammer-common Dependency

Ensure `swissarmyhammer-config/Cargo.toml` depends on swissarmyhammer-common:

```toml
[dependencies]
swissarmyhammer-common = { path = "../swissarmyhammer-common" }
```

### 2. Implement Severity for ConfigurationError

In `swissarmyhammer-config/src/error.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for ConfigurationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Cannot load or parse configuration
            ConfigurationError::InvalidFormat { .. } => ErrorSeverity::Critical,
            ConfigurationError::SchemaValidationFailed { .. } => ErrorSeverity::Critical,
            
            // Error: Configuration issues but can continue with defaults
            ConfigurationError::FileNotFound { .. } => ErrorSeverity::Error,
            ConfigurationError::MissingRequiredField { .. } => ErrorSeverity::Error,
            ConfigurationError::InvalidValue { .. } => ErrorSeverity::Error,
            
            // Warning: Non-critical config issues
            ConfigurationError::DeprecatedField { .. } => ErrorSeverity::Warning,
            ConfigurationError::UnknownField { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 3. Implement Severity for AgentError

In `swissarmyhammer-config/src/agent.rs`:

```rust
use swissarmyhammer_common::{ErrorSeverity, Severity};

impl Severity for AgentError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Agent system cannot function
            AgentError::SystemFailure { .. } => ErrorSeverity::Critical,
            AgentError::InvalidConfiguration { .. } => ErrorSeverity::Critical,
            
            // Error: Agent operation failed
            AgentError::AgentNotFound { .. } => ErrorSeverity::Error,
            AgentError::ExecutionFailed { .. } => ErrorSeverity::Error,
            AgentError::InvalidInput { .. } => ErrorSeverity::Error,
            
            // Warning: Agent issues but can continue
            AgentError::PerformanceDegraded { .. } => ErrorSeverity::Warning,
            AgentError::DeprecatedAgent { .. } => ErrorSeverity::Warning,
        }
    }
}
```

### 4. Add Tests

Create tests in both files:

```rust
#[cfg(test)]
mod severity_tests {
    use super::*;
    use swissarmyhammer_common::{ErrorSeverity, Severity};

    #[test]
    fn test_configuration_error_severity() {
        let error = ConfigurationError::InvalidFormat { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        let error = ConfigurationError::FileNotFound { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Error);
        
        let error = ConfigurationError::DeprecatedField { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_agent_error_severity() {
        let error = AgentError::SystemFailure { /* fields */ };
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        
        // Test other variants
    }
}
```

## Severity Guidelines for Config Errors

**Critical**:
- Invalid configuration format (YAML/JSON parse errors)
- Schema validation failures
- System-level agent failures

**Error**:
- Missing configuration files (can use defaults)
- Missing required fields
- Invalid configuration values
- Agent not found
- Agent execution failures

**Warning**:
- Deprecated configuration fields
- Unknown fields in configuration
- Performance degradation
- Deprecated agents

## Acceptance Criteria

- [ ] ConfigurationError implements Severity trait
- [ ] AgentError implements Severity trait
- [ ] Unit tests for both implementations
- [ ] Tests pass: `cargo test -p swissarmyhammer-config`
- [ ] Code compiles: `cargo build -p swissarmyhammer-config`
- [ ] Clippy clean: `cargo clippy -p swissarmyhammer-config`

## Files to Modify

- `swissarmyhammer-config/Cargo.toml` (add dependency if needed)
- `swissarmyhammer-config/src/error.rs` (implementation + tests)
- `swissarmyhammer-config/src/agent.rs` (implementation + tests)

## Estimated Changes

~80 lines of code (2 implementations + tests)

## Next Step

Step 6: Implement Severity for rules errors
