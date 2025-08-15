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