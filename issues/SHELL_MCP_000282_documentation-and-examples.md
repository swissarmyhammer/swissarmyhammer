# Documentation and Examples Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Create comprehensive documentation, usage examples, troubleshooting guides, and integration examples for the shell MCP tool to ensure easy adoption and effective usage.

## Objective

Provide complete documentation that covers installation, configuration, usage patterns, security considerations, and troubleshooting to enable users to effectively leverage the shell tool.

## Requirements

### User Documentation
- Complete tool usage documentation
- Configuration reference and examples
- Security best practices and guidelines
- Troubleshooting and FAQ sections
- Migration guides for existing users

### API Documentation
- MCP tool interface documentation
- Parameter specifications and validation
- Response format documentation
- Error codes and handling
- Integration patterns and examples

### Examples and Use Cases
- Common development workflow examples
- Build and CI/CD integration examples
- System administration examples
- Security-conscious usage examples
- Advanced configuration examples

### Integration Documentation
- CLI usage patterns and examples
- Workflow integration examples
- Configuration management examples
- MCP protocol integration examples
- Third-party tool integration patterns

## Implementation Details

### Tool Description Enhancement
```markdown
# MCP Shell Tool

Execute shell commands with comprehensive timeout controls and output capture.

## Description

The shell tool provides secure command execution with timeout management, working directory support, environment variable injection, and comprehensive output capture. Designed for development workflows, build automation, and system administration tasks.

## Parameters

- `command` (required): Shell command to execute
- `working_directory` (optional): Working directory for command execution
- `timeout` (optional): Command timeout in seconds (default: 300, max: 1800)
- `environment` (optional): Additional environment variables as key-value pairs

## Response Format

Successful execution returns:
- `content`: Status message and execution details
- `is_error`: Boolean indicating success/failure
- `metadata`: Comprehensive execution information including exit code, output, timing

## Security Features

- Command injection prevention
- Configurable directory access controls
- Audit logging for security compliance
- Resource usage monitoring and limits

## Examples

### Basic Command Execution
```json
{
  "command": "ls -la"
}
```

### Build with Environment Variables
```json
{
  "command": "cargo build --release",
  "working_directory": "/project",
  "timeout": 600,
  "environment": {
    "RUST_LOG": "info",
    "BUILD_ENV": "production"
  }
}
```
```

### CLI Usage Documentation
```markdown
# Shell Command Usage

The `sah shell` command provides direct access to the shell execution functionality.

## Basic Usage

```bash
# Execute simple commands
sah shell "echo 'Hello, World!'"

# Execute with working directory
sah shell -C /project "cargo test"

# Set timeout and environment variables
sah shell -t 600 -e "RUST_LOG=debug" -e "BUILD_ENV=test" "./build.sh"
```

## Options

- `-C, --directory <DIR>`: Set working directory
- `-t, --timeout <SECONDS>`: Set command timeout (default: 300, max: 1800)
- `-e, --env <KEY=VALUE>`: Set environment variables
- `--format <FORMAT>`: Output format (human, json, yaml)
- `--show-metadata`: Include execution metadata in output
- `-q, --quiet`: Suppress command output, show only results

## Examples

### Development Workflows

```bash
# Run tests with verbose output
sah shell -C /project -e "RUST_LOG=debug" "cargo test"

# Build with production settings
sah shell -t 900 -e "BUILD_ENV=production" "./scripts/build.sh"

# Check system status with metadata
sah shell --show-metadata "df -h && free -h"
```

### System Administration

```bash
# Monitor system resources
sah shell -t 60 "top -b -n 1"

# Generate system report
sah shell --format json "uname -a && lscpu"

# Process logs with timeout
sah shell -t 300 "tail -f /var/log/application.log | head -n 100"
```
```

### Configuration Documentation
```markdown
# Shell Tool Configuration

Configuration options for the shell tool can be set globally in `sah.toml` or overridden per execution.

## Configuration Schema

```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm -rf /", "format", "dd"]
allowed_directories = ["/project", "/tmp"]
max_command_length = 1000

[shell.output]
max_output_size = "10MB"
max_line_length = 2000
detect_binary_content = true

[shell.execution]
default_timeout = 300
max_timeout = 1800
min_timeout = 1

[shell.audit]
enable_audit_logging = true
log_command_output = false
```

## Environment Variables

Configuration can be overridden using environment variables:

- `SAH_SHELL_SECURITY_ENABLE_VALIDATION`: Enable/disable security validation
- `SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT`: Set default timeout
- `SAH_SHELL_OUTPUT_MAX_SIZE`: Set maximum output size
- `SAH_SHELL_SECURITY_ALLOWED_DIRECTORIES`: Comma-separated allowed directories

## Environment-Specific Configurations

### Development Environment
- Relaxed security settings for flexibility
- Higher timeout limits for long builds
- Verbose audit logging for debugging

### Production Environment  
- Strict security controls
- Resource usage limits
- Comprehensive audit logging

### CI/CD Environment
- Moderate security settings
- Optimized timeouts for build processes
- Structured logging for analysis
```

### Security Documentation
```markdown
# Shell Tool Security

The shell tool includes comprehensive security features to prevent malicious command execution.

## Security Features

### Command Injection Prevention
- Pattern-based dangerous command detection
- Input validation and sanitization
- Command length limitations

### Access Controls
- Directory access restrictions
- Command whitelist/blacklist support
- Environment variable filtering

### Audit Logging
- Comprehensive execution logging
- Security event tracking
- Compliance reporting support

## Security Best Practices

### For Development
- Use directory restrictions to limit access scope
- Enable audit logging for security monitoring
- Regularly review and update blocked command patterns

### For Production
- Enable strict security validation
- Use minimal allowed directories
- Implement comprehensive audit logging
- Monitor security events regularly

### Common Security Patterns

```bash
# Safe command execution with restrictions
sah shell -C /project "cargo build"

# Avoid dangerous patterns
# DON'T: sah shell "rm -rf *"
# DO: sah shell "cargo clean"
```

## Security Configuration Examples

```toml
# High-security production configuration
[shell.security]
enable_validation = true
blocked_commands = [
  "rm", "rmdir", "del", "format", "fdisk", 
  "mkfs", "dd", "sudo", "su"
]
allowed_directories = ["/app", "/tmp/app"]
max_command_length = 500
```
```

### Troubleshooting Guide
```markdown
# Shell Tool Troubleshooting

Common issues and solutions for the shell tool.

## Command Execution Issues

### Permission Denied
**Problem**: Commands fail with permission errors

**Solutions**:
- Check file permissions on executables
- Verify working directory access
- Review directory access restrictions in configuration
- Ensure proper environment variable settings

### Command Not Found
**Problem**: Shell reports command not found

**Solutions**:
- Verify command is in PATH
- Use absolute path to executable
- Check working directory setting
- Verify environment variable configuration

### Timeout Issues
**Problem**: Commands timeout unexpectedly

**Solutions**:
- Increase timeout setting for long-running commands
- Check system resource availability
- Review command complexity and optimization opportunities
- Consider breaking large commands into smaller steps

## Configuration Issues

### Invalid Configuration
**Problem**: Configuration file parsing errors

**Solutions**:
- Validate TOML syntax
- Check configuration schema against documentation
- Review environment variable formats
- Test configuration with minimal settings

### Security Validation Errors
**Problem**: Commands blocked by security validation

**Solutions**:
- Review blocked command patterns
- Check directory access restrictions
- Adjust security policy for environment needs
- Use alternative command approaches

## Performance Issues

### High Memory Usage
**Problem**: Shell tool consuming excessive memory

**Solutions**:
- Reduce output size limits
- Enable output truncation
- Review command output volume
- Check for memory leaks in long-running processes

### Slow Execution
**Problem**: Commands executing slower than expected

**Solutions**:
- Review system resource availability
- Check concurrent execution limits
- Optimize command complexity
- Consider command caching strategies

## Diagnostic Commands

```bash
# Check shell tool configuration
sah shell --show-metadata "echo 'Configuration test'"

# Test security validation
sah shell "echo 'Security test'"

# Monitor resource usage
sah shell -t 60 "ps aux | grep sah"
```
```

### Integration Examples
```markdown
# Shell Tool Integration Examples

Examples of integrating the shell tool with various systems and workflows.

## Workflow Integration

```yaml
# workflow.yml
name: "build_and_test"
states:
  - name: "setup"
    actions:
      - type: "shell"
        command: "git clean -fd"
        working_directory: "/project"
        
  - name: "build"  
    actions:
      - type: "shell"
        command: "cargo build --release"
        timeout: 900
        environment:
          RUST_LOG: "info"
          BUILD_ENV: "production"
          
  - name: "test"
    actions:
      - type: "shell"
        command: "cargo test"
        capture_output: true
        fail_on_error: true
```

## MCP Client Integration

```javascript
// JavaScript MCP client example
const client = new MCPClient();

const result = await client.callTool('shell_execute', {
  command: 'npm run build',
  working_directory: '/project',
  timeout: 600,
  environment: {
    NODE_ENV: 'production'
  }
});

console.log('Build result:', result.metadata);
```

## CI/CD Integration

```yaml
# GitHub Actions example
- name: Execute shell command via SAH
  run: |
    sah shell -t 900 -e "CI=true" -e "NODE_ENV=production" "npm run build"
```
```

## Acceptance Criteria

- [ ] Complete tool documentation with examples
- [ ] Configuration reference with all options
- [ ] Security best practices documented
- [ ] Troubleshooting guide comprehensive
- [ ] Integration examples for common use cases
- [ ] API documentation complete and accurate
- [ ] Examples tested and verified working
- [ ] Documentation integrated with existing docs

## Documentation Structure

```
doc/src/
├── shell-tool/
│   ├── introduction.md
│   ├── installation.md
│   ├── usage.md
│   ├── configuration.md
│   ├── security.md
│   ├── troubleshooting.md
│   ├── examples/
│   │   ├── development.md
│   │   ├── ci-cd.md
│   │   ├── system-admin.md
│   │   └── advanced.md
│   └── integration/
│       ├── workflow.md
│       ├── mcp-client.md
│       └── cli.md
└── SUMMARY.md  # Update to include shell tool docs
```

## Notes

- Documentation should be comprehensive but approachable
- Examples should be practical and tested
- Security documentation is critical for adoption confidence
- Troubleshooting guide should cover common real-world issues
- Integration examples help users understand practical applications
- Keep documentation updated with any functionality changes