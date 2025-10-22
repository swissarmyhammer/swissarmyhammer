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


## Proposed Solution

After analyzing the actual error variants in the codebase:

### ConfigurationError Severity Mapping

Based on actual variants in `swissarmyhammer-config/src/error.rs`:

- **Critical**: `FigmentError`, `LoadError` - Core configuration system failures that prevent system operation
- **Error**: `DiscoveryError`, `EnvVarError`, `TemplateContextError`, `IoError`, `JsonError` - Configuration issues that affect functionality but allow fallback/defaults

**Rationale**: 
- `LoadError` and `FigmentError` are critical because they represent fundamental configuration system failures
- Other errors affect specific operations but the system can potentially continue with defaults

### AgentError Severity Mapping

Based on actual variants in `swissarmyhammer-config/src/agent.rs`:

- **Critical**: `ParseError`, `ConfigError` - Cannot parse or validate agent configuration
- **Error**: `NotFound`, `InvalidPath`, `IoError` - Agent operations failed but system can continue

**Rationale**:
- Parse and config validation failures are critical because they prevent using the agent entirely
- Not finding an agent or path issues are errors but allow fallback to other agents

### Implementation Plan

1. Add Severity trait implementation to ConfigurationError in error.rs
2. Add comprehensive tests for ConfigurationError severity
3. Add Severity trait implementation to AgentError in agent.rs
4. Add comprehensive tests for AgentError severity
5. Verify compilation and all tests pass



## Implementation Notes

### Completed Tasks

1. **ConfigurationError Severity Implementation** (swissarmyhammer-config/src/error.rs:92-107)
   - Added `use swissarmyhammer_common::{ErrorSeverity, Severity};` import
   - Implemented Severity trait with the following mappings:
     - **Critical**: `LoadError`, `FigmentError` - Core configuration system failures
     - **Error**: `DiscoveryError`, `EnvVarError`, `TemplateContextError`, `IoError`, `JsonError` - Configuration issues that allow fallback

2. **ConfigurationError Tests** (swissarmyhammer-config/src/error.rs:109-146)
   - Added 7 comprehensive tests covering all error variants
   - Tests verify correct severity levels for each error type
   - Fixed type ambiguity in `test_figment_error_is_critical` by explicitly typing intermediate variable

3. **AgentError Severity Implementation** (swissarmyhammer-config/src/agent.rs:489-502)
   - Added `use swissarmyhammer_common::{ErrorSeverity, Severity};` import
   - Implemented Severity trait with the following mappings:
     - **Critical**: `ParseError`, `ConfigError` - Cannot parse or validate agent configuration
     - **Error**: `NotFound`, `InvalidPath`, `IoError` - Agent operations failed but system can continue

4. **AgentError Tests** (swissarmyhammer-config/src/agent.rs:2617-2651)
   - Added 5 comprehensive tests covering all error variants
   - Tests verify correct severity levels for each error type

### Test Results

- **Build**: ✅ Successful (`cargo build -p swissarmyhammer-config`)
- **Tests**: ✅ All 235 tests passed (`cargo nextest run -p swissarmyhammer-config`)
- **Clippy**: ✅ No warnings (`cargo clippy -p swissarmyhammer-config`)

### Design Decisions

1. **Critical vs Error Distinction**:
   - Critical errors prevent the configuration/agent system from functioning at all
   - Error-level issues affect specific operations but allow the system to continue with fallbacks or defaults

2. **No Warning-level errors**: 
   - The existing error types in both enums represent actual failures, not warnings
   - Future additions could introduce warning-level variants for deprecations or non-critical issues

3. **Test Coverage**:
   - Every error variant is tested
   - Tests use realistic error construction patterns matching production code
