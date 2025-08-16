# MCP Shell Tool Specification

## Overview

This specification defines a new MCP tool `shell` that enables LLMs to execute shell commands in a controlled environment. The tool provides secure command execution with timeout controls and proper output handling for interactive AI workflows.

## Problem Statement

LLMs often need to execute shell commands to:
1. Check system state and environment
2. Run build and test commands 
3. Perform file operations and data processing
4. Interact with development tools and utilities
5. Execute scripts and automation tasks

Currently, there's no standardized way for MCP tools to execute arbitrary shell commands with proper safety controls and timeout management.

## Solution: MCP Shell Tool

### Tool Definition

**Tool Name**: `shell`  
**Purpose**: Execute shell commands with timeout and output capture  
**Usage Context**: Available to LLMs during MCP workflow execution

### Parameters

```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "The shell command to execute",
      "minLength": 1
    },
    "working_directory": {
      "type": "string", 
      "description": "Working directory for command execution (optional, defaults to current directory)"
    },
    "timeout": {
      "type": "integer",
      "description": "Command timeout in seconds (optional, defaults to 300 seconds / 5 minutes)",
      "minimum": 1,
      "maximum": 1800,
      "default": 300
    },
    "environment": {
      "type": "object",
      "description": "Additional environment variables to set (optional)",
      "additionalProperties": {
        "type": "string"
      }
    }
  },
  "required": ["command"]
}
```

### Implementation Requirements

#### Command Execution
- Execute commands in a separate process using `std::process::Command`
- Capture both stdout and stderr output
- Return exit code along with output
- Support setting working directory and environment variables
- Handle command interruption on timeout

#### Timeout Management
- **Default Timeout**: 5 minutes (300 seconds)
- **Maximum Timeout**: 30 minutes (1800 seconds) 
- **Minimum Timeout**: 1 second
- Use `tokio::time::timeout` for async command execution
- Kill process tree on timeout to prevent orphaned processes
- Return timeout error with partial output if available

#### Security Controls
- Validate command strings to prevent shell injection
- Restrict access to sensitive directories if configured
- Log all executed commands for audit trails
- Optional command whitelist/blacklist support

#### Output Handling
- Stream output in real-time for long-running commands
- Truncate excessive output to prevent memory issues
- Handle binary output gracefully
- Preserve exit codes and signal information

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

## Use Cases

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
- Sanitize command strings to prevent injection attacks
- Optional regex-based command filtering
- Logging of all executed commands with timestamps

### Resource Protection
- Process isolation using system-level controls
- Memory and CPU usage monitoring
- Disk space protection through working directory restrictions

### Access Controls
- Optional directory access restrictions
- Environment variable filtering
- User permission inheritance

## Configuration Options

### Global Settings
```toml
[shell_tool]
default_timeout = 300  # seconds
max_timeout = 1800     # seconds
max_output_size = "10MB"
allowed_directories = ["/project", "/tmp"]
blocked_commands = ["rm -rf", "format", "dd"]
log_commands = true
```

### Per-Execution Settings
- Timeout overrides (within limits)
- Working directory specification
- Environment variable injection
- Output size limits

## Error Handling

### Command Not Found
- Return clear error message
- Suggest similar commands if available
- Log attempt for security audit

### Permission Denied
- Return permission error with context
- Log security violation
- Suggest alternative approaches

### Resource Exhaustion  
- Handle out-of-memory conditions
- Manage disk space limits
- Control CPU usage spikes

## Integration with Existing Tools

### Workflow Integration
- Shell commands can be part of larger workflows
- Output can be passed to subsequent tools
- Conditional execution based on exit codes

### Logging Integration
- All commands logged through tracing system
- Structured logging with execution metadata
- Security audit trail maintenance

### Error Propagation
- Shell failures can trigger workflow abort files
- Integration with existing MCP error handling
- Graceful degradation on command failures

## Implementation Strategy

### Phase 1: Basic Implementation
- Core command execution with timeout
- Basic output capture and formatting
- Simple error handling and logging

### Phase 2: Security Enhancements
- Command validation and filtering
- Directory access restrictions
- Enhanced logging and auditing

### Phase 3: Advanced Features
- Real-time output streaming
- Interactive command support
- Enhanced resource monitoring

## Inspiration and References

### QwenLM Shell Tool
Based on the shell tool implementation from [Qwen Code](https://github.com/QwenLM/qwen-code/blob/main/packages/core/src/tools/shell.ts), this specification adapts the concept for Rust/MCP environment with enhanced security and timeout controls.

### Key Adaptations
- Rust-native process management using `std::process::Command`
- Tokio async runtime integration for timeout handling  
- MCP-specific response format and error handling
- Enhanced security controls for server environments

## Testing Strategy

### Unit Tests
- Command execution with various parameters
- Timeout behavior verification
- Error condition handling
- Output formatting validation

### Integration Tests  
- Workflow integration scenarios
- Security control verification
- Resource limit testing
- Cross-platform compatibility

### Security Tests
- Command injection prevention
- Resource exhaustion protection
- Access control validation
- Audit trail verification

## Future Enhancements

### Interactive Shell Support
- Persistent shell sessions
- Interactive command input/output
- Shell state preservation across commands

### Enhanced Output Processing
- Syntax highlighting for command output
- Structured data extraction from output
- Real-time progress indicators

### Advanced Security
- Sandboxed execution environments
- Container-based isolation
- Advanced threat detection

## Conclusion

The MCP shell tool provides essential command execution capabilities for LLM workflows while maintaining security and resource controls. The 5-minute default timeout balances practical usage needs with system protection, and the comprehensive parameter set enables flexible usage across diverse scenarios.