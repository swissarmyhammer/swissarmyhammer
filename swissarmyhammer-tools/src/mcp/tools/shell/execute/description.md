# Shell Execute Tool

Execute shell commands with timeout controls and proper output handling for interactive AI workflows.

## Purpose

The shell execute tool provides secure command execution capabilities for LLMs, enabling them to:
- Check system state and environment
- Run build and test commands 
- Perform file operations and data processing
- Interact with development tools and utilities
- Execute scripts and automation tasks

## Parameters

- `command` (required): The shell command to execute
  - Type: string
  - Description: Shell command string to run (e.g., "ls -la", "cargo test")
  - Minimum length: 1 character
  
- `working_directory` (optional): Working directory for command execution
  - Type: string
  - Description: Path where the command should be executed
  - Default: Current working directory
  
- `timeout` (optional): Command timeout in seconds
  - Type: integer  
  - Description: Maximum time to wait for command completion
  - Default: 300 seconds (5 minutes)
  - Range: 1 to 1800 seconds (30 minutes)
  
- `environment` (optional): Additional environment variables
  - Type: object
  - Description: Key-value pairs of environment variables to set
  - Properties: Additional string properties allowed

## Response Format

### Successful Execution
```json
{
  "content": [{
    "type": "text",
    "text": "Command executed successfully"
  }],
  "is_error": false,
  "metadata": {
    "command": "ls -la",
    "exit_code": 0,
    "stdout": "total 8\ndrwxr-xr-x  3 user  staff   96 Jan 15 10:30 .\n...",
    "stderr": "",
    "execution_time_ms": 45,
    "working_directory": "/path/to/dir"
  }
}
```

### Command Failure
```json
{
  "content": [{
    "type": "text", 
    "text": "Command failed with exit code 1"
  }],
  "is_error": true,
  "metadata": {
    "command": "ls /nonexistent",
    "exit_code": 1,
    "stdout": "",
    "stderr": "ls: /nonexistent: No such file or directory\n",
    "execution_time_ms": 23,
    "working_directory": "/path/to/dir"
  }
}
```

### Timeout Error
```json
{
  "content": [{
    "type": "text",
    "text": "Command timed out after 300 seconds"
  }],
  "is_error": true, 
  "metadata": {
    "command": "long_running_command",
    "timeout_seconds": 300,
    "partial_stdout": "...",
    "partial_stderr": "...",
    "working_directory": "/path/to/dir"
  }
}
```

## Usage Examples

### Development Workflow
```json
{
  "command": "cargo test",
  "working_directory": "/project/path",
  "timeout": 600
}
```

### System Information  
```json
{
  "command": "uname -a && df -h",
  "timeout": 30
}
```

### Build Automation
```json
{
  "command": "./build.sh --release",
  "environment": {
    "RUST_LOG": "debug",
    "BUILD_ENV": "production"
  },
  "timeout": 900
}
```

### File Operations
```json
{
  "command": "find . -name '*.rs' -type f | wc -l",
  "working_directory": "/src"
}
```

## Security Considerations

### Command Validation
- Commands are executed in isolated processes
- All executed commands are logged with timestamps
- Optional command filtering through configuration
- Input sanitization to prevent injection attacks

### Resource Protection
- Configurable timeout limits prevent runaway processes
- Process isolation using system-level controls
- Memory and CPU usage monitoring capabilities
- Working directory restrictions when configured

### Access Controls
- Inherits user permissions for command execution
- Optional directory access restrictions
- Environment variable filtering capabilities
- Comprehensive audit trail for security compliance

## Error Handling

The tool handles various error conditions gracefully:

- **Command Not Found**: Clear error message with command details
- **Permission Denied**: Detailed permission error with context
- **Timeout**: Partial output returned when available
- **Resource Exhaustion**: Controlled failure with diagnostic information
- **Invalid Parameters**: Comprehensive validation error messages

## Integration Notes

- Integrates with existing MCP error handling patterns
- Uses structured logging through the tracing system
- Supports workflow integration for complex automation
- Output can be passed to subsequent tools in workflows
- Compatible with SwissArmyHammer's abort mechanism for workflow control

## Implementation Status

This is the foundational infrastructure setup for the shell execute tool. The actual command execution engine will be implemented in subsequent phases following Test Driven Development principles.