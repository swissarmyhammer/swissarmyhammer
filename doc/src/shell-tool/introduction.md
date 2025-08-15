# Shell Tool Introduction

The SwissArmyHammer shell tool enables secure, controlled execution of shell commands within AI workflows. It provides a standardized interface for command execution with comprehensive safety controls, making it suitable for development automation, system administration, and build processes.

## Why Use the Shell Tool?

### For AI Workflows
- **Controlled Environment**: Execute commands with predictable timeouts and resource limits
- **Security First**: Built-in command validation and access controls
- **Structured Output**: Consistent JSON responses with detailed execution metadata
- **Error Handling**: Graceful failure handling with comprehensive error information

### For Development
- **Build Integration**: Execute build commands, tests, and deployment scripts
- **Environment Management**: Control working directories and environment variables
- **Development Tools**: Interact with Git, package managers, and development utilities
- **CI/CD Integration**: Reliable command execution for automated workflows

### For System Administration  
- **System Monitoring**: Check system status, resources, and service health
- **Automation**: Execute maintenance scripts and administrative tasks
- **Diagnostics**: Run diagnostic commands with timeout protection
- **Compliance**: Audit logging for security and compliance requirements

## Core Concepts

### Command Execution Model
The shell tool executes commands in isolated processes, capturing all output and providing detailed execution metadata. Each command execution is independent, with no state carried between executions.

### Security Model
- **Input Validation**: Commands are validated to prevent injection attacks
- **Process Isolation**: Commands run in separate processes with controlled environments
- **Access Controls**: Optional restrictions on directory access and command patterns
- **Audit Trail**: All executions logged with timestamps and execution details

### Resource Management
- **Timeout Controls**: Configurable timeouts prevent runaway processes
- **Memory Limits**: Output size limits prevent memory exhaustion
- **Process Cleanup**: Automatic cleanup of child processes on timeout
- **Resource Monitoring**: Track execution time and system resource usage

## Installation and Setup

The shell tool is included with SwissArmyHammer and requires no additional installation. It's available both as a CLI command and as an MCP tool.

### Prerequisites
- SwissArmyHammer installed and configured
- Appropriate system permissions for desired commands
- Optional: Configuration file for security settings

### Verification
Test the shell tool installation:

```bash
# Basic test
sah shell "echo 'Shell tool is working'"

# Test with metadata
sah shell --show-metadata "date"

# Test timeout handling  
sah shell -t 5 "sleep 2 && echo 'Success'"
```

## Basic Usage Patterns

### Development Commands
```bash
# Run tests
sah shell -C /project "cargo test"

# Build with environment variables
sah shell -e "RUST_LOG=debug" "cargo build --release"

# Check Git status
sah shell "git status --porcelain"
```

### System Information
```bash
# System status
sah shell "uname -a && uptime"

# Disk usage
sah shell "df -h"

# Process information
sah shell "ps aux | head -10"
```

### File Operations
```bash
# Find files
sah shell "find . -name '*.rs' -type f | wc -l"

# Directory listing with details
sah shell -C /var/log "ls -la *.log"

# File content analysis
sah shell "grep -r 'ERROR' logs/ | wc -l"
```

## Output Formats

The shell tool supports multiple output formats for different use cases:

### Human Format (Default)
Displays command output naturally with execution status.

### JSON Format  
Structured data suitable for programmatic processing:
```bash
sah shell --format json "date"
```

### YAML Format
Human-readable structured output:
```bash
sah shell --format yaml "whoami"
```

## Integration Points

### MCP Protocol
The shell tool implements the MCP tool interface, making it available to any MCP-compatible client.

### Workflow System
Integration with SwissArmyHammer workflows allows shell commands to be part of larger automation sequences.

### Configuration System
Comprehensive configuration options for security, performance, and operational settings.

## Security Considerations

### Safe Command Practices
- Use absolute paths when possible
- Avoid commands that modify critical system files
- Set appropriate timeouts for expected execution duration
- Use working directory restrictions for additional safety

### Environment Isolation
- Environment variables are isolated per execution
- Working directories can be restricted to safe locations
- Process cleanup prevents resource leaks

### Audit and Monitoring
- All commands logged with execution details
- Security events tracked for compliance
- Failed attempts recorded for security analysis

## Next Steps

Continue with detailed guides for specific use cases:

- **[CLI Usage](cli-usage.md)** - Complete CLI command reference
- **[Configuration](configuration.md)** - Security and performance settings  
- **[Examples](examples/)** - Practical usage examples
- **[Security](security.md)** - Comprehensive security guidelines
- **[Troubleshooting](troubleshooting.md)** - Common issues and solutions

## Getting Help

If you encounter issues or have questions:

1. Check the [troubleshooting guide](troubleshooting.md)
2. Review [security best practices](security.md)  
3. Examine [practical examples](examples/) for similar use cases
4. Consult the [configuration reference](configuration.md) for settings