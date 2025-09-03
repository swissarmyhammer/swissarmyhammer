# Shell Tool

The shell tool provides secure command execution capabilities for AI workflows, enabling automated command execution with comprehensive timeout controls, security validation, and proper output handling.

## Overview

The shell tool bridges the gap between AI interactions and system command execution, allowing LLMs to:
- Execute development commands (build, test, lint)
- Perform file operations and system queries
- Run automation scripts and deployment tasks
- Interact with development tools and utilities
- Check system state and environment information

## Key Features

- **Secure Execution**: Command validation and security controls prevent malicious command execution
- **Timeout Management**: Configurable timeouts (1 second to 30 minutes) prevent runaway processes
- **Environment Control**: Support for custom environment variables and working directories
- **Comprehensive Output**: Captures stdout, stderr, exit codes, and execution timing
- **Audit Logging**: All commands logged with timestamps for security compliance
- **Error Handling**: Graceful handling of failures, timeouts, and resource limitations

## Quick Start

### Basic Usage

Execute a simple command:
```bash
sah shell "echo 'Hello, World!'"
```

Run a command in a specific directory:
```bash
sah shell -C /project "cargo test"
```

Set environment variables and timeout:
```bash
sah shell -t 600 -e "RUST_LOG=debug" -e "BUILD_ENV=test" "./build.sh"
```

### MCP Tool Usage

The shell tool is available as an MCP tool named `shell_execute`:

```json
{
  "command": "ls -la",
  "working_directory": "/project",
  "timeout": 30,
  "environment": {
    "RUST_LOG": "info"
  }
}
```

## Tool Parameters

- **command** (required): Shell command to execute
  - Type: string
  - Example: `"cargo build --release"`

- **working_directory** (optional): Working directory for execution  
  - Type: string
  - Default: Current directory
  - Example: `"/project/src"`

- **timeout** (optional): Command timeout in seconds
  - Type: integer
  - Default: 300 (5 minutes)
  - Range: 1 to 1800 (30 minutes)

- **environment** (optional): Additional environment variables
  - Type: object (key-value pairs)
  - Example: `{"RUST_LOG": "debug", "NODE_ENV": "production"}`

## Response Format

The shell tool returns structured responses with execution details:

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

### Error Response
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

## Security Features

- **Command Validation**: Input sanitization prevents command injection
- **Access Controls**: Optional directory restrictions limit file system access
- **Resource Limits**: Timeout and output size controls prevent resource exhaustion  
- **Audit Logging**: Comprehensive logging for security monitoring and compliance
- **Process Isolation**: Commands execute in separate processes with controlled environments

## Next Steps

- [CLI Usage Guide](shell-tool/cli-usage.md) - Detailed CLI command reference
- [Configuration](shell-tool/configuration.md) - Security and execution settings  
- [Examples](shell-tool/examples/) - Practical usage examples
- [Security Best Practices](shell-tool/security.md) - Safe command execution guidelines
- [Troubleshooting](shell-tool/troubleshooting.md) - Common issues and solutions