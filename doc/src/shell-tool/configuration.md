# Shell Tool Configuration

The shell tool provides comprehensive configuration options to control security, execution behavior, output handling, and audit logging. Configuration is managed through the SwissArmyHammer configuration system using TOML files.

## Configuration Overview

Shell tool configuration is organized into four main sections:
- **Security**: Command validation and access controls
- **Output**: Output size limits and handling strategies  
- **Execution**: Timeout and process management settings
- **Audit**: Logging and compliance configuration

## Configuration File Structure

Add shell tool configuration to your `sah.toml` file:

```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm -rf /", "format", "dd if=", "mkfs", "fdisk"]
allowed_directories = ["/project", "/tmp", "/home/user/workspace"]
max_command_length = 1000
enable_injection_detection = true

[shell.output]
max_output_size = "10MB"
max_line_length = 2000
detect_binary_content = true
truncation_strategy = "PreserveStructure"

[shell.execution]
default_timeout = 300
max_timeout = 1800
min_timeout = 1
cleanup_process_tree = true

[shell.audit]
enable_audit_logging = true
log_level = "info"
log_command_output = false
max_audit_entry_size = 10000
```

## Security Configuration

Controls command validation, access restrictions, and injection prevention.

### `enable_validation` (boolean)
**Default**: `true`

Enables comprehensive security validation of commands before execution.

```toml
[shell.security]
enable_validation = true
```

### `blocked_commands` (array of strings)
**Default**: `["rm -rf /", "format", "dd if=", "mkfs", "fdisk"]`

List of command patterns that are blocked from execution.

```toml
[shell.security]
blocked_commands = [
    "rm -rf /",          # Prevent recursive deletion of root
    "format",            # Block disk formatting commands  
    "dd if=",            # Block dangerous dd operations
    "mkfs",              # Block filesystem creation
    "fdisk",             # Block disk partitioning
    "sudo rm",           # Block privileged deletions
    "> /dev/",           # Block device overwriting
]
```

### `allowed_directories` (optional array of strings)
**Default**: `null` (no restrictions)

Restricts command execution to specified directories only. When set, commands can only be executed in these directories or their subdirectories.

```toml
[shell.security]
# Restrict to safe directories only
allowed_directories = [
    "/home/user/projects",
    "/tmp",
    "/opt/workspace",
    "/var/tmp"
]
```

### `max_command_length` (integer)
**Default**: `1000`

Maximum allowed command length in characters.

```toml
[shell.security]
max_command_length = 1000  # Standard limit
# max_command_length = 500   # Stricter limit for production
# max_command_length = 2000  # More permissive for complex commands
```

### `enable_injection_detection` (boolean)
**Default**: `true`

Enables pattern-based detection of potential command injection attempts.

```toml
[shell.security]
enable_injection_detection = true
```

## Output Configuration

Controls output capture, size limits, and handling strategies.

### `max_output_size` (string)
**Default**: `"10MB"`

Maximum size of captured command output before truncation. Supports units: B, KB, MB, GB.

```toml
[shell.output]
max_output_size = "10MB"  # Default for most use cases
# max_output_size = "1MB"   # Conservative for memory-constrained environments
# max_output_size = "50MB"  # Higher limit for data processing commands
# max_output_size = "100KB" # Very conservative for security-sensitive environments
```

### `max_line_length` (integer)
**Default**: `2000`

Maximum length of individual output lines before truncation.

```toml
[shell.output]
max_line_length = 2000  # Default for most terminal output
# max_line_length = 1000  # Conservative for display formatting
# max_line_length = 5000  # Higher limit for data processing
```

### `detect_binary_content` (boolean)
**Default**: `true`

Enables detection and special handling of binary content in command output.

```toml
[shell.output]
detect_binary_content = true
```

### `truncation_strategy` (string)
**Default**: `"PreserveStructure"`

Strategy for truncating output when size limits are exceeded:
- `"PreserveStructure"`: Maintain line boundaries when truncating
- `"SimpleTruncation"`: Simple byte-based truncation
- `"WordBoundary"`: Truncate at word boundaries when possible

```toml
[shell.output]
truncation_strategy = "PreserveStructure"  # Best for log files and structured output
# truncation_strategy = "WordBoundary"      # Good for text processing
# truncation_strategy = "SimpleTruncation"  # Fastest, least intelligent
```

## Execution Configuration

Controls command execution timeouts and process management.

### `default_timeout` (integer)
**Default**: `300` (5 minutes)

Default timeout in seconds for commands that don't specify a timeout.

```toml
[shell.execution]
default_timeout = 300   # 5 minutes - good balance for most commands
# default_timeout = 60    # 1 minute - for quick commands only
# default_timeout = 900   # 15 minutes - for longer build processes
```

### `max_timeout` (integer)
**Default**: `1800` (30 minutes)

Maximum allowed timeout in seconds, even when explicitly requested.

```toml
[shell.execution]
max_timeout = 1800      # 30 minutes - default maximum
# max_timeout = 3600      # 1 hour - for very long processes
# max_timeout = 600       # 10 minutes - conservative limit
```

### `min_timeout` (integer)
**Default**: `1`

Minimum allowed timeout in seconds.

```toml
[shell.execution]
min_timeout = 1         # 1 second minimum
# min_timeout = 5         # 5 second minimum for slower systems
```

### `cleanup_process_tree` (boolean)
**Default**: `true`

Enable cleanup of child processes when commands timeout or fail.

```toml
[shell.execution]
cleanup_process_tree = true
```

## Audit Configuration

Controls logging and audit trail generation.

### `enable_audit_logging` (boolean)
**Default**: `false`

Enable comprehensive audit logging of all shell command executions.

```toml
[shell.audit]
enable_audit_logging = true   # Enable for production environments
# enable_audit_logging = false  # Disable for development to reduce noise
```

### `log_level` (string)
**Default**: `"info"`

Log level for audit entries. Options: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`.

```toml
[shell.audit]
log_level = "info"      # Standard audit logging
# log_level = "debug"     # Detailed debugging information
# log_level = "warn"      # Only warnings and errors
```

### `log_command_output` (boolean)
**Default**: `false`

Include command output in audit logs (disabled by default for security and size reasons).

```toml
[shell.audit]
log_command_output = false  # Default - output not logged for security
# log_command_output = true   # Enable for detailed debugging (security risk)
```

### `max_audit_entry_size` (integer)
**Default**: `10000` (10KB)

Maximum size of individual audit log entries in bytes.

```toml
[shell.audit]
max_audit_entry_size = 10000  # 10KB default
# max_audit_entry_size = 5000   # 5KB for conservative logging
# max_audit_entry_size = 20000  # 20KB for detailed logging
```

## Environment-Specific Configurations

### Development Environment
Relaxed settings for development productivity:

```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm -rf /", "format"]  # Minimal blocking
allowed_directories = ["/home/dev", "/tmp", "/opt/projects"]
max_command_length = 2000
enable_injection_detection = true

[shell.output]
max_output_size = "50MB"  # Higher limit for build outputs
max_line_length = 5000
detect_binary_content = true
truncation_strategy = "PreserveStructure"

[shell.execution]
default_timeout = 600   # 10 minutes for development builds
max_timeout = 3600      # 1 hour maximum
min_timeout = 1
cleanup_process_tree = true

[shell.audit]
enable_audit_logging = false  # Reduced noise in development
log_level = "info"
log_command_output = false
max_audit_entry_size = 10000
```

### Production Environment
Strict security and comprehensive auditing:

```toml
[shell.security]
enable_validation = true
blocked_commands = [
    "rm", "rmdir", "del", "format", "fdisk", "mkfs", "dd",
    "sudo", "su", "chmod 777", "chown", "mount", "umount"
]
allowed_directories = ["/app", "/tmp/app", "/opt/safe"]
max_command_length = 500
enable_injection_detection = true

[shell.output]
max_output_size = "1MB"   # Conservative output limits
max_line_length = 1000
detect_binary_content = true
truncation_strategy = "PreserveStructure"

[shell.execution]
default_timeout = 300
max_timeout = 900         # 15 minutes maximum
min_timeout = 5
cleanup_process_tree = true

[shell.audit]
enable_audit_logging = true   # Comprehensive auditing
log_level = "info"
log_command_output = false    # Security: don't log sensitive output
max_audit_entry_size = 5000
```

### CI/CD Environment
Optimized for build processes:

```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm -rf /", "format", "fdisk"]
allowed_directories = ["/builds", "/cache", "/tmp"]
max_command_length = 1500
enable_injection_detection = true

[shell.output]
max_output_size = "100MB"  # Build outputs can be large
max_line_length = 2000
detect_binary_content = true
truncation_strategy = "PreserveStructure"

[shell.execution]
default_timeout = 900      # 15 minutes for builds
max_timeout = 3600         # 1 hour for complex builds
min_timeout = 1
cleanup_process_tree = true

[shell.audit]
enable_audit_logging = true
log_level = "info"
log_command_output = false
max_audit_entry_size = 15000  # Larger entries for build info
```

## Environment Variable Overrides

Configuration can be overridden using environment variables:

```bash
# Security settings
export SAH_SHELL_SECURITY_ENABLE_VALIDATION=true
export SAH_SHELL_SECURITY_MAX_COMMAND_LENGTH=1000
export SAH_SHELL_SECURITY_ALLOWED_DIRECTORIES="/safe/dir1,/safe/dir2"

# Execution settings
export SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT=300
export SAH_SHELL_EXECUTION_MAX_TIMEOUT=1800

# Output settings
export SAH_SHELL_OUTPUT_MAX_OUTPUT_SIZE="10MB"
export SAH_SHELL_OUTPUT_MAX_LINE_LENGTH=2000

# Audit settings
export SAH_SHELL_AUDIT_ENABLE_AUDIT_LOGGING=true
export SAH_SHELL_AUDIT_LOG_LEVEL=info
```

## Validation and Testing

### Validate Configuration
Test your configuration with a simple command:

```bash
# Test basic execution
sah shell "echo 'Configuration test'"

# Test timeout settings
sah shell -t 10 "sleep 5 && echo 'Timeout test passed'"

# Test directory restrictions (if configured)
sah shell -C /allowed/directory "pwd"
```

### Debug Configuration Issues
Use metadata to understand configuration behavior:

```bash
# Check execution with full metadata
sah shell --show-metadata --format json "echo 'Debug test'"

# Test security validation
sah shell "echo 'Testing security validation'"

# Test output limits
sah shell "for i in {1..1000}; do echo 'Line $i'; done"
```

## Security Best Practices

### Command Blocking
- Block dangerous commands that could damage the system
- Include variations and common aliases of dangerous commands
- Consider the execution context when defining blocks

### Directory Restrictions  
- Use absolute paths for directory restrictions
- Include only necessary directories
- Consider temporary directory needs for build processes

### Output Limits
- Set conservative limits in production environments
- Consider memory usage when setting output limits
- Monitor actual output sizes to tune limits appropriately

### Audit Logging
- Enable audit logging in production environments
- Be cautious about logging sensitive command output
- Regularly review audit logs for security incidents
- Implement log rotation for audit files

## Troubleshooting

### Configuration Loading Issues
- Verify TOML syntax with a validator
- Check file permissions on configuration file
- Use environment variables to override problematic settings
- Enable debug logging to see configuration loading process

### Security Validation Errors
- Review blocked command patterns for false positives
- Check directory restrictions for necessary paths
- Verify command length limits are appropriate
- Test injection detection with legitimate commands

### Performance Issues
- Adjust output size limits for better memory usage
- Consider timeout settings for system performance
- Monitor audit log size and rotation
- Optimize truncation strategy for use case

For more troubleshooting information, see the [Troubleshooting Guide](troubleshooting.md).