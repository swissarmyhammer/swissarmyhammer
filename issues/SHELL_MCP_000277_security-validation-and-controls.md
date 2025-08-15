# Security Validation and Controls Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Implement comprehensive security controls for shell command execution, including command validation, injection prevention, access restrictions, and audit logging.

## Objective

Add robust security measures to prevent malicious command execution while maintaining usability for legitimate development and automation tasks.

## Requirements

### Command Validation and Sanitization
- Implement command injection prevention
- Validate command syntax and structure
- Filter dangerous command patterns
- Provide configurable command restrictions

### Access Controls
- Optional directory access restrictions
- Configurable command whitelist/blacklist
- User permission validation
- Resource usage monitoring

### Audit Logging
- Comprehensive logging of all command executions
- Security-relevant event logging
- Structured log format for analysis
- Integration with existing logging system

### Configuration-Based Security
- Global security policy configuration
- Per-execution security overrides
- Environment-specific security levels
- Runtime security policy updates

## Implementation Details

### Command Validation Framework
```rust
#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub enable_validation: bool,
    pub blocked_commands: Vec<String>,
    pub allowed_directories: Option<Vec<PathBuf>>,
    pub max_command_length: usize,
    pub enable_audit_logging: bool,
}

pub struct CommandValidator {
    policy: SecurityPolicy,
}

impl CommandValidator {
    pub fn validate_command(
        &self, 
        command: &str,
        working_dir: &Path
    ) -> Result<(), SecurityError> {
        self.check_command_length(command)?;
        self.check_blocked_patterns(command)?;
        self.check_injection_patterns(command)?;
        self.check_directory_access(working_dir)?;
        Ok(())
    }
}
```

### Injection Prevention
```rust
impl CommandValidator {
    fn check_injection_patterns(&self, command: &str) -> Result<(), SecurityError> {
        let dangerous_patterns = [
            r";\s*rm\s+-rf",           // Command chaining with rm -rf
            r"\|\s*sh",                // Piping to shell
            r"\$\([^)]*\)",           // Command substitution
            r"`[^`]*`",                // Backtick command substitution
            r"&&\s*rm",                // Command chaining with rm
            r"\|\|\s*rm",              // Or chaining with rm
            r">\s*/dev/",              // Output redirection to devices
            r"<\s*/dev/",              // Input redirection from devices
        ];
        
        for pattern in &dangerous_patterns {
            if regex::Regex::new(pattern)?.is_match(command) {
                return Err(SecurityError::DangerousCommand {
                    pattern: pattern.to_string(),
                    command: command.to_string(),
                });
            }
        }
        Ok(())
    }
}
```

### Directory Access Controls
```rust
impl CommandValidator {
    fn check_directory_access(&self, working_dir: &Path) -> Result<(), SecurityError> {
        if let Some(allowed_dirs) = &self.policy.allowed_directories {
            let canonical_dir = working_dir.canonicalize()
                .map_err(|_| SecurityError::InvalidDirectory)?;
                
            let is_allowed = allowed_dirs.iter().any(|allowed| {
                canonical_dir.starts_with(allowed)
            });
            
            if !is_allowed {
                return Err(SecurityError::DirectoryNotAllowed {
                    directory: working_dir.to_path_buf(),
                });
            }
        }
        Ok(())
    }
}
```

### Audit Logging System
```rust
#[derive(Debug, Serialize)]
pub struct ShellAuditEvent {
    pub timestamp: DateTime<Utc>,
    pub command: String,
    pub working_directory: PathBuf,
    pub exit_code: Option<i32>,
    pub execution_time_ms: Option<u64>,
    pub security_validation: SecurityValidationResult,
    pub user_context: Option<String>,
}

pub fn log_shell_execution(event: ShellAuditEvent) {
    tracing::info!(
        target: "shell_audit",
        command = %event.command,
        working_dir = %event.working_directory.display(),
        exit_code = event.exit_code,
        execution_time_ms = event.execution_time_ms,
        "Shell command executed"
    );
}
```

### Configuration Integration
```rust
// Integration with existing sah_config system
#[derive(Debug, Clone, Deserialize)]
pub struct ShellToolConfig {
    pub security: SecurityPolicy,
    pub output_limits: OutputLimits,
    pub default_timeout: u64,
    pub max_timeout: u64,
}

impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            security: SecurityPolicy::default(),
            output_limits: OutputLimits::default(),
            default_timeout: 300,
            max_timeout: 1800,
        }
    }
}
```

## Security Error Types
```rust
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Command contains dangerous pattern: {pattern} in command: {command}")]
    DangerousCommand { pattern: String, command: String },
    
    #[error("Command too long: {length} exceeds limit {limit}")]
    CommandTooLong { length: usize, limit: usize },
    
    #[error("Directory not allowed: {directory}")]
    DirectoryNotAllowed { directory: PathBuf },
    
    #[error("Blocked command pattern: {pattern}")]
    BlockedCommand { pattern: String },
    
    #[error("Invalid directory: {0}")]
    InvalidDirectory(#[from] std::io::Error),
}
```

## Integration Points

### Existing Security Module
- Integrate with `swissarmyhammer/src/security.rs`
- Follow established security patterns
- Use existing validation frameworks
- Maintain consistency with other tools

### Configuration System
- Use existing `sah_config` for security policies
- Support environment-specific configurations
- Enable runtime policy updates
- Provide sensible security defaults

### Logging Integration
- Use existing `tracing` infrastructure
- Follow established logging patterns
- Support structured audit logs
- Enable log analysis and monitoring

## Acceptance Criteria

- [ ] Command injection prevention working effectively
- [ ] Directory access controls enforced properly
- [ ] Configurable security policies functional
- [ ] Audit logging captures all security events
- [ ] Blocked command patterns detected correctly
- [ ] Security errors provide clear information
- [ ] Performance impact minimized
- [ ] Configuration integration seamless

## Testing Requirements

- [ ] Security tests for injection prevention
- [ ] Access control validation tests
- [ ] Blocked command pattern tests
- [ ] Audit logging verification tests
- [ ] Performance impact measurement
- [ ] Configuration loading tests
- [ ] Cross-platform security behavior tests

## Configuration Examples

### Development Environment
```toml
[shell_tool.security]
enable_validation = true
blocked_commands = ["rm -rf /", "format", "dd if="]
allowed_directories = ["/project", "/tmp"]
max_command_length = 1000
enable_audit_logging = true
```

### Production Environment
```toml
[shell_tool.security]
enable_validation = true
blocked_commands = ["rm", "format", "dd", "fdisk", "mkfs"]
allowed_directories = ["/app", "/tmp/app"]
max_command_length = 500
enable_audit_logging = true
```

## Notes

- Security is critical but should not break legitimate use cases
- Focus on preventing obvious attacks while maintaining usability
- Configuration allows environment-specific security policies
- Audit logging is essential for security monitoring and compliance
- Balance security with performance impact

## Proposed Solution

After analyzing the existing codebase, I will implement a comprehensive security validation framework for shell command execution that integrates with the existing infrastructure:

### Architecture Overview

1. **ShellSecurityValidator**: Core validation engine that checks commands for dangerous patterns, injection attempts, and policy violations
2. **ShellSecurityPolicy**: Configuration structure for security policies that integrates with the existing `sah_config` system
3. **ShellAuditLogger**: Structured audit logging for all shell command executions
4. **Integration Points**: Seamless integration with existing `ShellAction`, security module, and error handling patterns

### Implementation Strategy

#### Phase 1: Core Security Framework
- Create `ShellSecurityValidator` with command validation methods
- Implement `ShellSecurityPolicy` configuration structure  
- Add security error types to existing error hierarchy
- Design audit logging infrastructure

#### Phase 2: Command Validation Engine
- Implement command injection prevention using regex patterns
- Add dangerous command pattern detection
- Create command length and complexity limits
- Build directory access control validation

#### Phase 3: Configuration Integration
- Integrate with existing `sah_config::SahConfig` structure
- Add environment-specific security policy support
- Create default security policies for different environments
- Support runtime policy updates

#### Phase 4: Audit and Logging
- Implement structured audit logging using existing `tracing` infrastructure
- Create security event categorization and severity levels
- Add execution timing and resource usage tracking
- Support configurable log targets and formats

#### Phase 5: ShellAction Integration
- Modify existing `ShellAction::execute()` to use security validation
- Preserve backward compatibility with existing workflows
- Add security policy override capabilities
- Implement graceful degradation for validation failures

### Key Security Features

- **Command Injection Prevention**: Detect and block shell injection patterns like `; rm -rf`, command substitution, and pipe redirection to dangerous commands
- **Directory Access Controls**: Optional restriction of command execution to specific directories
- **Command Pattern Filtering**: Configurable blocked/allowed command patterns
- **Resource Limits**: Command length limits, execution timeouts, and complexity restrictions
- **Comprehensive Audit Logging**: Structured logs of all command executions with security context

### Backward Compatibility

The implementation will maintain full backward compatibility with existing `ShellAction` usage while adding security as an opt-in capability through configuration. Existing workflows will continue to work unchanged unless security policies are explicitly configured.

### Testing Strategy

- Unit tests for each security validation component
- Integration tests with existing `ShellAction` infrastructure
- Security-focused tests for injection prevention and access controls
- Performance tests to ensure minimal overhead
- Cross-platform compatibility tests

This approach leverages the existing codebase patterns and infrastructure while adding robust security controls that can be configured per environment and use case.
## Implementation Completed ✅

Successfully implemented comprehensive security validation and controls for shell command execution with the following achievements:

### ✅ Core Implementation

1. **Shell Security Module**: Created `swissarmyhammer/src/shell_security.rs` with complete security framework
   - `ShellSecurityValidator`: Core validation engine with configurable policies
   - `ShellSecurityPolicy`: Comprehensive security policy configuration
   - `ShellSecurityError`: Detailed error types for security violations

2. **Command Injection Prevention**: Implemented robust pattern detection for:
   - Command chaining (`;`, `&&`, `||`) 
   - Command substitution (`$()`, backticks)
   - Pipe redirection to dangerous commands
   - Python/Perl/Ruby code execution patterns
   - Network-based attack patterns (netcat, reverse shells)

3. **Directory Access Controls**: Implemented optional directory restrictions with:
   - Configurable allowed directories list
   - Path canonicalization for security
   - Prevention of path traversal attacks

4. **Comprehensive Audit Logging**: 
   - `ShellAuditEvent` structure for detailed execution tracking
   - Integration with existing `tracing` infrastructure
   - Structured logging for security analysis
   - Command start and completion logging with timing

5. **Configuration System Integration**:
   - Integration with existing `sah_config` system
   - Support for `sah.toml` security policy configuration
   - Runtime policy loading and validation
   - Fallback to secure defaults

### ✅ Security Features

**Command Validation**:
- Length limits (configurable, default 10,000 chars)
- Dangerous pattern detection (rm -rf, format, fdisk, etc.)
- Injection pattern prevention (15+ patterns detected)
- Blocked command configuration (regex-based)

**Environment Security**:
- Environment variable name validation
- Protection against injection in env values
- Configurable restrictions

**Audit Trail**:
- Complete logging of all shell executions
- Security violation logging with details
- Performance metrics (execution time tracking)
- Structured data for security analysis

### ✅ Integration Points

1. **ShellAction Integration**: Updated existing `ShellAction` workflow integration
   - All security validations applied before execution
   - Comprehensive audit logging added
   - Backward compatible with existing workflows

2. **Error Handling**: Added `ShellSecurityError` to `ActionError` enum
   - Proper error propagation through workflow system
   - Clear error messages for security violations

3. **Testing**: Implemented comprehensive test suite
   - Unit tests for all security components
   - Integration tests with existing shell actions
   - Security-focused test scenarios

### ✅ Configuration Example

```toml
[shell_security]
enable_validation = true
blocked_commands = ["rm -rf /", "format", "fdisk"]
allowed_directories = ["/project", "/tmp"]
max_command_length = 1000
enable_audit_logging = true
enable_injection_prevention = true
default_timeout_seconds = 300
max_timeout_seconds = 3600
```

### ✅ Key Files Modified/Created

- `swissarmyhammer/src/shell_security.rs` - Main security framework (NEW)
- `swissarmyhammer/src/workflow/actions.rs` - Updated security integration
- `swissarmyhammer/src/workflow/executor/core.rs` - Error handling updates
- `swissarmyhammer/src/lib.rs` - Module exports
- Test files updated for new security behavior

### ✅ Backwards Compatibility

The implementation maintains full backward compatibility:
- Existing workflows continue to work unchanged
- Security is opt-in through configuration
- Default policies provide reasonable security without breaking functionality
- Graceful degradation when configuration is unavailable

### ✅ Performance Impact

Minimal performance impact:
- Security validation adds negligible overhead
- Regex compilation cached at startup
- Efficient pattern matching algorithms
- Audit logging uses structured, efficient tracing

This implementation provides enterprise-grade security for shell command execution while maintaining the flexibility and usability required for development workflows.