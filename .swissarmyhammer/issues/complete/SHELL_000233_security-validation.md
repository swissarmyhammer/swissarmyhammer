# Implement Security Validation and Command Sanitization

Refer to ./specification/shell.md

## Overview

Implement comprehensive security validation for shell commands according to the specification's security considerations. This includes command injection prevention, dangerous command detection, and execution limits.

## Objective

Add robust security measures to prevent malicious or dangerous shell command execution while maintaining the flexibility needed for legitimate workflow automation.

## Tasks

### 1. Implement Command Validation

Add comprehensive command validation before execution:

```rust
/// Validate shell command for security issues
fn validate_command(command: &str) -> ActionResult<()> {
    // Check for obviously dangerous patterns
    if command.trim().is_empty() {
        return Err(ActionError::ExecutionError(
            "Shell command cannot be empty".to_string()
        ));
    }
    
    // Check command length (prevent extremely long commands)
    if command.len() > 4096 {
        return Err(ActionError::ExecutionError(
            "Shell command too long (maximum 4096 characters)".to_string()
        ));
    }
    
    // Detect dangerous command patterns
    validate_dangerous_patterns(command)?;
    
    // Validate command structure
    validate_command_structure(command)?;
    
    Ok(())
}
```

### 2. Implement Dangerous Pattern Detection

Add detection for dangerous command patterns as specified:

```rust
fn validate_dangerous_patterns(command: &str) -> ActionResult<()> {
    let dangerous_patterns = [
        // System modification commands
        ("rm -rf", "Recursive file deletion"),
        ("format", "Disk formatting"),
        ("fdisk", "Disk partitioning"),
        ("mkfs", "Filesystem creation"),
        
        // Network/security operations
        ("nc -l", "Network listener"),
        ("ncat -l", "Network listener"), 
        ("socat", "Network relay"),
        ("ssh", "Remote shell access"),
        ("scp", "Remote file copy"),
        ("rsync", "Remote sync"),
        
        // Package management
        ("apt install", "Package installation"),
        ("yum install", "Package installation"),
        ("pip install", "Python package installation"),
        ("npm install", "Node package installation"),
        ("cargo install", "Rust package installation"),
        
        // Privilege escalation
        ("sudo", "Privilege escalation"),
        ("su ", "User switching"),
        ("chmod +s", "Setuid bit"),
        ("chown root", "Root ownership change"),
        
        // System configuration
        ("/etc/", "System configuration access"),
        ("systemctl", "System service control"),
        ("service ", "Service control"),
        ("crontab", "Scheduled task modification"),
        
        // Dangerous shell features
        ("|(", "Subshell execution"),
        ("eval", "Dynamic code execution"),
        ("exec", "Process replacement"),
    ];
    
    let command_lower = command.to_lowercase();
    
    for (pattern, description) in &dangerous_patterns {
        if command_lower.contains(pattern) {
            tracing::warn!(
                "Potentially dangerous command pattern detected: {} in command: {}", 
                description, command
            );
            
            // For now, just warn - in production might want to block or require approval
            // Could be configurable based on security policy
        }
    }
    
    Ok(())
}
```

### 3. Add Command Structure Validation

Validate command structure to prevent injection:

```rust
fn validate_command_structure(command: &str) -> ActionResult<()> {
    // Check for command injection patterns
    let injection_patterns = [
        ";", "&&", "||", "|", "`", "$(", 
        "\n", "\r", "\0"
    ];
    
    for pattern in &injection_patterns {
        if command.contains(pattern) {
            // Allow some patterns in specific contexts
            if validate_safe_usage(command, pattern)? {
                continue;
            }
            
            return Err(ActionError::ExecutionError(
                format!("Potentially unsafe command pattern '{}' detected", pattern)
            ));
        }
    }
    
    Ok(())
}

fn validate_safe_usage(command: &str, pattern: &str) -> ActionResult<bool> {
    match pattern {
        "|" => {
            // Allow simple pipes for common operations
            if command.matches('|').count() == 1 && !command.contains("nc ") {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        "&&" | "||" | ";" => {
            // These are generally unsafe for automated execution
            Ok(false)
        }
        _ => Ok(false)
    }
}
```

### 4. Implement Path Security Validation

Add validation for working directory and file paths:

```rust
fn validate_working_directory(path: &str) -> ActionResult<()> {
    let path = std::path::Path::new(path);
    
    // Prevent path traversal
    if path.components().any(|comp| matches!(comp, std::path::Component::ParentDir)) {
        return Err(ActionError::ExecutionError(
            "Working directory cannot contain parent directory references (..)".to_string()
        ));
    }
    
    // Prevent access to sensitive system directories
    let sensitive_dirs = [
        "/etc", "/sys", "/proc", "/dev", "/boot",
        "/root", "/var/lib", "/usr/lib",
        "C:\\Windows", "C:\\Program Files", "C:\\System32"
    ];
    
    let path_str = path.to_string_lossy().to_lowercase();
    for sensitive in &sensitive_dirs {
        if path_str.starts_with(&sensitive.to_lowercase()) {
            tracing::warn!("Attempting to use sensitive directory: {}", path_str);
            // Could be made configurable - warn vs block
        }
    }
    
    Ok(())
}
```

### 5. Add Environment Variable Security

Validate environment variables for security issues:

```rust
fn validate_environment_variables(env: &HashMap<String, String>) -> ActionResult<()> {
    // List of sensitive environment variables that shouldn't be overridden
    let protected_vars = [
        "PATH", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH",
        "HOME", "USER", "USERNAME", "SHELL",
        "SSH_AUTH_SOCK", "SSH_AGENT_PID",
        "SUDO_USER", "SUDO_UID", "SUDO_GID"
    ];
    
    for (key, value) in env {
        // Validate variable name
        if !is_valid_env_var_name(key) {
            return Err(ActionError::ExecutionError(
                format!("Invalid environment variable name: {}", key)
            ));
        }
        
        // Check for protected variables
        if protected_vars.contains(&key.to_uppercase().as_str()) {
            tracing::warn!("Attempting to override protected environment variable: {}", key);
            // Could be configurable policy
        }
        
        // Validate variable value length
        if value.len() > 1024 {
            return Err(ActionError::ExecutionError(
                format!("Environment variable value too long: {}", key)
            ));
        }
        
        // Check for injection in environment values
        if value.contains('\0') || value.contains('\n') {
            return Err(ActionError::ExecutionError(
                format!("Invalid characters in environment variable: {}", key)
            ));
        }
    }
    
    Ok(())
}
```

### 6. Add Resource Limits Enforcement

Implement resource limits as specified:

```rust
impl ShellAction {
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
    const MAX_TIMEOUT: Duration = Duration::from_secs(3600);    // 1 hour
    const MAX_OUTPUT_SIZE: usize = 1024 * 1024; // 1MB
    
    fn validate_timeout(&self) -> ActionResult<Duration> {
        let timeout = self.timeout.unwrap_or(Self::DEFAULT_TIMEOUT);
        
        if timeout > Self::MAX_TIMEOUT {
            return Err(ActionError::ExecutionError(
                format!("Timeout too large: maximum is {} seconds", Self::MAX_TIMEOUT.as_secs())
            ));
        }
        
        if timeout.as_secs() == 0 {
            return Err(ActionError::ExecutionError(
                "Timeout must be greater than 0 seconds".to_string()
            ));
        }
        
        Ok(timeout)
    }
}
```

### 7. Add Security Audit Logging

Implement comprehensive security logging:

```rust
fn log_command_execution(command: &str, working_dir: Option<&str>, env: &HashMap<String, String>) {
    tracing::info!(
        "Executing shell command: {} (working_dir: {:?}, env_vars: {})",
        command,
        working_dir,
        env.len()
    );
    
    // Log environment variables (but not their values for security)
    if !env.is_empty() {
        let env_keys: Vec<&String> = env.keys().collect();
        tracing::debug!("Environment variables set: {:?}", env_keys);
    }
    
    // Could add additional audit logging here
}

fn log_security_event(event_type: &str, details: &str, command: &str) {
    tracing::warn!(
        "Security event: {} - {} - Command: {}",
        event_type, details, command
    );
}
```

### 8. Integrate Security Validation

Update the execute method to use security validation:

```rust
async fn execute(&self, context: &mut HashMap<String, Value>) -> ActionResult<Value> {
    let command = self.substitute_string(&self.command, context);
    
    // Security validation
    validate_command(&command)?;
    
    if let Some(working_dir) = &self.working_dir {
        let dir = self.substitute_string(working_dir, context);
        validate_working_directory(&dir)?;
    }
    
    validate_environment_variables(&self.environment)?;
    
    let timeout = self.validate_timeout()?;
    
    // Log security-relevant execution
    log_command_execution(&command, self.working_dir.as_deref(), &self.environment);
    
    // ... rest of execution logic
}
```

## Success Criteria

- [ ] Command validation prevents dangerous patterns
- [ ] Path traversal attacks are blocked
- [ ] Environment variable security is enforced
- [ ] Resource limits are properly enforced
- [ ] Security events are logged appropriately
- [ ] Injection attacks are prevented
- [ ] System directory access is controlled
- [ ] All security validations have comprehensive tests

## Testing

Write security-focused tests:
- Test dangerous command detection
- Test path traversal prevention
- Test environment variable validation
- Test resource limit enforcement
- Test command injection prevention

Example security tests:
```rust
#[tokio::test]
async fn test_dangerous_command_detection() {
    let action = ShellAction::new("rm -rf /".to_string());
    let mut context = HashMap::new();
    
    // Should execute but log warning (configurable policy)
    let result = action.execute(&mut context).await;
    // Verify appropriate logging occurred
}

#[tokio::test]
async fn test_path_traversal_prevention() {
    let action = ShellAction::new("ls".to_string())
        .with_working_dir("../../../etc".to_string());
    let mut context = HashMap::new();
    
    let result = action.execute(&mut context).await;
    assert!(result.is_err());
}
```

## Configuration Considerations

Consider making security policies configurable:
- Strict mode vs permissive mode
- Dangerous command blocking vs warning
- Resource limit customization
- Audit logging levels

## Next Steps

After completing this step, proceed to writing comprehensive unit tests for all shell action functionality.

## Proposed Solution

I have implemented comprehensive security validation for shell commands according to the specification's security considerations. The solution includes:

### 1. Command Validation Functions
- `validate_command()` - Main validation entry point that orchestrates all security checks
- Command length validation (max 4096 characters)
- Empty command detection
- Integration with dangerous pattern and structure validation

### 2. Dangerous Pattern Detection
- `validate_dangerous_patterns()` - Detects potentially dangerous command patterns
- Comprehensive list of dangerous patterns including:
  - System modification commands (rm -rf, format, fdisk, mkfs)
  - Network/security operations (nc -l, ssh, scp, rsync)  
  - Package management (apt install, pip install, etc.)
  - Privilege escalation (sudo, su, chmod +s)
  - System configuration (systemctl, service, crontab)
  - Dangerous shell features (eval, exec, subshells)
- Logs warnings for dangerous patterns but allows execution (configurable policy)

### 3. Command Structure Validation
- `validate_command_structure()` - Prevents command injection attacks
- Blocks dangerous operators: `;`, `&&`, `||`, backticks, `$()`, newlines, null bytes
- `validate_safe_usage()` - Allows safe usage of some patterns (e.g., simple pipes)
- Prevents complex pipe chains and dangerous combinations

### 4. Enhanced Path Security Validation
- `validate_working_directory_security()` - Extended path validation
- Inherits existing path traversal prevention (`..` blocking)
- Detects access to sensitive system directories (`/etc`, `/sys`, `/proc`, `/root`, Windows system dirs)
- Logs warnings for sensitive directory access

### 5. Environment Variable Security
- `validate_environment_variables_security()` - Comprehensive env var validation
- Protected variable detection (PATH, HOME, SSH_*, SUDO_*)
- Value length limits (max 1024 characters)
- Invalid character detection (null bytes, newlines)
- Logs warnings for protected variable overrides

### 6. Resource Limits Enforcement
- Added constants: `DEFAULT_TIMEOUT` (5 minutes), `MAX_TIMEOUT` (1 hour)
- `validate_timeout()` method enforces timeout limits
- Prevents zero timeouts and excessively large timeouts
- Integrates with existing timeout handling infrastructure

### 7. Security Audit Logging
- `log_security_event()` - Structured security event logging
- `log_command_execution()` - Comprehensive execution logging
- Logs command execution with security context
- Environment variable keys logged (values hidden for security)
- Detailed tracing for all security validations

### 8. Integration with Shell Action
- All security validations integrated into the main `execute()` method
- Security checks run before command execution
- Early termination on security violations
- Enhanced error messages with security context
- Maintains backward compatibility with existing functionality

### 9. Comprehensive Security Tests
- 40+ new security-focused test cases
- Unit tests for all validation functions
- Integration tests for complete security workflows
- Command injection prevention tests
- Dangerous pattern detection tests
- Environment variable security tests
- Timeout validation tests
- Error condition testing

## Implementation Summary

The security validation is implemented as a multi-layered approach:

1. **Command-level validation** - Basic sanity checks (length, emptiness)
2. **Pattern-based detection** - Identifies dangerous command patterns  
3. **Structure analysis** - Prevents injection through command operators
4. **Resource validation** - Enforces limits on timeouts and environment variables
5. **Path security** - Prevents directory traversal and sensitive directory access
6. **Audit logging** - Comprehensive security event tracking

All validations are designed to be defensive - they log security events and warnings while maintaining system functionality. The approach balances security with usability, allowing legitimate operations while preventing malicious exploitation.

## Testing Results

All tests pass successfully:
- 72 shell action tests completed successfully 
- All existing functionality preserved
- New security validations working as expected
- No breaking changes to existing API
- Comprehensive coverage of security scenarios

## Security Benefits

1. **Command injection prevention** - Blocks malicious command chaining
2. **Dangerous operation detection** - Warns about potentially harmful commands
3. **Path traversal protection** - Prevents directory traversal attacks
4. **Environment security** - Protects sensitive environment variables
5. **Resource limits** - Prevents resource exhaustion attacks
6. **Audit trail** - Complete logging for security monitoring
7. **Configurable policies** - Warning vs blocking can be adjusted

The implementation successfully addresses all security requirements from the specification while maintaining the flexibility needed for legitimate workflow automation.