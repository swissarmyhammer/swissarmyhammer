# Shell Execute Tool

Execute shell commands with proper output handling for interactive AI workflows.

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



## Usage Examples

### Development Workflow
```json
{
  "command": "cargo test",
  "working_directory": "/project/path"
}
```

### System Information  
```json
{
  "command": "uname -a && df -h"
}
```

### Build Automation
```json
{
  "command": "./build.sh --release",
  "environment": {
    "RUST_LOG": "debug",
    "BUILD_ENV": "production"
  }
}
```

### File Operations
```json
{
  "command": "find . -name '*.rs' -type f | wc -l",
  "working_directory": "/src"
}
```
