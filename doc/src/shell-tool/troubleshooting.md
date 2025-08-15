# Shell Tool Troubleshooting

This guide covers common issues, error scenarios, and solutions when using the shell tool. It includes diagnostic techniques, performance optimization, and resolution strategies for various problems.

## Common Issues

### Command Execution Failures

#### Command Not Found

**Problem**: Commands fail with "command not found" errors.

**Symptoms**:
```bash
$ sah shell "nonexistent-command"
Command failed with exit code 127
/bin/sh: nonexistent-command: command not found
```

**Solutions**:
1. **Verify command availability**:
   ```bash
   # Check if command exists in PATH
   sah shell "which your-command"
   
   # List available commands in PATH
   sah shell "echo $PATH | tr ':' '\n' | xargs ls"
   ```

2. **Use absolute paths**:
   ```bash
   # Instead of: sah shell "node"
   sah shell "/usr/bin/node"
   
   # Find absolute path
   sah shell "which node"
   ```

3. **Check working directory**:
   ```bash
   # Command might be in a specific directory
   sah shell -C /project/bin "your-command"
   
   # Add to PATH temporarily
   sah shell -e "PATH=/custom/bin:$PATH" "your-command"
   ```

#### Permission Denied

**Problem**: Commands fail due to insufficient permissions.

**Symptoms**:
```bash
$ sah shell "cat /etc/shadow"
Command failed with exit code 1
cat: /etc/shadow: Permission denied
```

**Solutions**:
1. **Check file permissions**:
   ```bash
   sah shell "ls -la /path/to/file"
   ```

2. **Verify user permissions**:
   ```bash
   sah shell "whoami"
   sah shell "id"
   sah shell "groups"
   ```

3. **Use appropriate user context**:
   ```bash
   # Check if sudo is available (if allowed)
   sah shell "sudo -l"
   
   # Use alternative approaches
   sah shell "stat /etc/shadow"  # Get file info without reading
   ```

4. **Fix file permissions if appropriate**:
   ```bash
   sah shell "chmod +x script.sh"  # Make script executable
   sah shell "chown user:group file"  # Change ownership if permitted
   ```

#### Working Directory Issues

**Problem**: Commands fail because they're executed in the wrong directory.

**Symptoms**:
```bash
$ sah shell "cargo build"
Command failed with exit code 101
error: could not find `Cargo.toml`
```

**Solutions**:
1. **Specify working directory**:
   ```bash
   # Use -C flag to set working directory
   sah shell -C /project "cargo build"
   ```

2. **Verify current directory**:
   ```bash
   sah shell "pwd"
   sah shell "ls -la"
   ```

3. **Use absolute paths**:
   ```bash
   sah shell "cd /project && cargo build"
   ```

### Timeout Issues

#### Commands Timing Out

**Problem**: Commands exceed timeout limits and are killed.

**Symptoms**:
```bash
$ sah shell -t 30 "sleep 60"
Command timed out after 30 seconds
Partial output may be available in metadata
```

**Solutions**:
1. **Increase timeout**:
   ```bash
   # Increase timeout for long operations
   sah shell -t 900 "long-running-build.sh"
   ```

2. **Optimize commands**:
   ```bash
   # Break large operations into smaller steps
   sah shell -t 300 "step1.sh"
   sah shell -t 300 "step2.sh"
   
   # Use parallel processing
   sah shell -t 600 "make -j$(nproc)"
   ```

3. **Monitor progress**:
   ```bash
   # Add progress indicators to scripts
   sah shell -t 1800 "./build.sh --verbose"
   
   # Check resource usage
   sah shell --show-metadata "resource-intensive-command"
   ```

#### Timeout Configuration Issues

**Problem**: Timeout limits are too restrictive or permissive.

**Solutions**:
1. **Check configuration**:
   ```toml
   [shell.execution]
   default_timeout = 300
   max_timeout = 1800
   min_timeout = 1
   ```

2. **Adjust per environment**:
   ```bash
   # Development: higher limits
   export SAH_SHELL_EXECUTION_MAX_TIMEOUT=3600
   
   # Production: conservative limits
   export SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT=300
   ```

3. **Monitor execution times**:
   ```bash
   sah shell --show-metadata "your-command" | grep execution_time_ms
   ```

### Security Validation Errors

#### Commands Blocked by Security

**Problem**: Legitimate commands are blocked by security validation.

**Symptoms**:
```bash
$ sah shell "rm temp-file.txt"
Command blocked by security validation: 'rm' matches blocked pattern
```

**Solutions**:
1. **Use alternative commands**:
   ```bash
   # Instead of rm, use safer alternatives
   sah shell "trash temp-file.txt"  # If trash command available
   sah shell "mv temp-file.txt /tmp/"  # Move to temp directory
   ```

2. **Adjust security configuration**:
   ```toml
   [shell.security]
   blocked_commands = [
       "rm -rf /",    # Keep dangerous patterns
       # Remove "rm" if needed for legitimate use
   ]
   ```

3. **Use specific patterns**:
   ```toml
   [shell.security]
   blocked_commands = [
       "rm -rf /",        # Block recursive root deletion
       "rm -rf /*",       # Block root deletion patterns
       "rm -rf /$",       # Block root deletion
       # Don't block all "rm" usage
   ]
   ```

#### Directory Access Denied

**Problem**: Commands are blocked due to directory restrictions.

**Solutions**:
1. **Check allowed directories**:
   ```toml
   [shell.security]
   allowed_directories = ["/project", "/tmp", "/home/user/workspace"]
   ```

2. **Use allowed directories**:
   ```bash
   # Copy files to allowed directory
   sah shell "cp /restricted/file /tmp/"
   sah shell -C /tmp "process-file file"
   ```

3. **Adjust directory restrictions**:
   ```toml
   [shell.security]
   allowed_directories = [
       "/project",
       "/tmp", 
       "/home/user",
       "/opt/workspace"  # Add needed directories
   ]
   ```

### Output and Performance Issues

#### Large Output Truncation

**Problem**: Command output is truncated due to size limits.

**Symptoms**:
```bash
$ sah shell "find / -type f"
[... truncated output ...]
Output truncated due to size limit (10MB)
```

**Solutions**:
1. **Increase output limits**:
   ```toml
   [shell.output]
   max_output_size = "50MB"  # Increase limit
   max_line_length = 5000    # Increase line length
   ```

2. **Filter output**:
   ```bash
   # Reduce output size
   sah shell "find /project -type f | head -100"
   
   # Use specific filters
   sah shell "find /project -name '*.rs' -type f"
   ```

3. **Redirect to files**:
   ```bash
   # Write output to file instead
   sah shell "find / -type f > /tmp/file-list.txt"
   sah shell "wc -l /tmp/file-list.txt"
   ```

#### Memory Usage Issues

**Problem**: Shell tool consumes excessive memory.

**Solutions**:
1. **Reduce output capture**:
   ```toml
   [shell.output]
   max_output_size = "1MB"     # Conservative limit
   max_line_length = 1000      # Shorter lines
   ```

2. **Use streaming commands**:
   ```bash
   # Instead of loading all output
   sah shell "large-data-command | head -n 100"
   
   # Process in chunks
   sah shell "split -l 1000 large-file.txt chunk-"
   ```

3. **Monitor resource usage**:
   ```bash
   sah shell --show-metadata "memory-intensive-command"
   ```

### Environment and Configuration Issues

#### Environment Variables Not Set

**Problem**: Commands fail because required environment variables are missing.

**Solutions**:
1. **Set environment variables**:
   ```bash
   sah shell -e "RUST_LOG=debug" -e "PATH=/custom/bin:$PATH" "your-command"
   ```

2. **Check current environment**:
   ```bash
   sah shell "env | sort"
   sah shell "echo $PATH"
   ```

3. **Load from environment file**:
   ```bash
   # Load environment variables from file
   source .env && sah shell -e "VAR1=$VAR1" -e "VAR2=$VAR2" "command"
   ```

#### Configuration Loading Problems

**Problem**: Shell tool configuration is not loaded correctly.

**Solutions**:
1. **Verify configuration file**:
   ```bash
   # Check configuration file syntax
   toml-lint sah.toml
   
   # Validate configuration
   sah validate
   ```

2. **Check file permissions**:
   ```bash
   ls -la sah.toml
   chmod 644 sah.toml  # Fix permissions if needed
   ```

3. **Use environment variable overrides**:
   ```bash
   # Override problematic configuration
   export SAH_SHELL_EXECUTION_DEFAULT_TIMEOUT=600
   export SAH_SHELL_SECURITY_ENABLE_VALIDATION=true
   ```

### Process Management Issues

#### Orphaned Processes

**Problem**: Child processes are not cleaned up properly.

**Solutions**:
1. **Enable process tree cleanup**:
   ```toml
   [shell.execution]
   cleanup_process_tree = true
   ```

2. **Check for orphaned processes**:
   ```bash
   # Check for orphaned processes
   ps aux | grep -E "(defunct|zombie)"
   
   # Check process tree
   pstree -p
   ```

3. **Manual cleanup if needed**:
   ```bash
   # Kill orphaned processes (be careful!)
   pkill -f "orphaned-process-name"
   ```

#### Resource Exhaustion

**Problem**: System resources are exhausted by shell commands.

**Solutions**:
1. **Set resource limits**:
   ```toml
   [shell.execution]
   default_timeout = 300
   max_timeout = 1800
   
   [shell.output] 
   max_output_size = "10MB"
   ```

2. **Monitor resource usage**:
   ```bash
   # Check system resources
   sah shell "free -h"
   sah shell "df -h"
   sah shell "uptime"
   ```

3. **Optimize commands**:
   ```bash
   # Use resource-efficient alternatives
   sah shell "find . -type f | wc -l"  # Instead of storing all files
   sah shell "du -sh ."  # Instead of detailed file listing
   ```

## Diagnostic Techniques

### Debug Command Execution

**View detailed execution information**:
```bash
# Show all metadata
sah shell --show-metadata --format json "your-command"

# Check execution timing
sah shell --show-metadata "time-sensitive-command" | jq '.metadata.execution_time_ms'

# View all output and error information
sah shell --format yaml "failing-command"
```

### Test Configuration

**Validate security settings**:
```bash
# Test command blocking
sah shell "echo 'This should work'"
sah shell "rm -rf /"  # Should be blocked

# Test directory restrictions
sah shell -C /allowed/dir "pwd"
sah shell -C /restricted/dir "pwd"  # May be blocked
```

**Test resource limits**:
```bash
# Test timeout
sah shell -t 5 "sleep 2"   # Should succeed
sah shell -t 5 "sleep 10"  # Should timeout

# Test output limits
sah shell "echo 'short output'"
sah shell "for i in {1..10000}; do echo \$i; done"  # May be truncated
```

### Performance Analysis

**Monitor execution performance**:
```bash
# Time command execution
sah shell --show-metadata "performance-test-command"

# Check resource usage during execution
# (Run in separate terminal)
watch "ps aux | grep sah"
```

**Identify bottlenecks**:
```bash
# Profile disk I/O
sah shell "iotop -a -o -d 1"

# Profile CPU usage
sah shell "top -b -n 1"

# Check memory usage
sah shell "vmstat 1 5"
```

## Error Messages Reference

### Exit Codes

| Exit Code | Meaning | Common Causes |
|-----------|---------|---------------|
| 0 | Success | Command completed successfully |
| 1 | General error | Command failed, permission denied, file not found |
| 2 | Misuse of shell builtin | Invalid command usage, syntax errors |
| 126 | Command invoked cannot execute | Permission problems, not an executable |
| 127 | Command not found | Command not in PATH, typo in command name |
| 128 | Invalid argument to exit | Script called `exit` with invalid argument |
| 130 | Script terminated by Control-C | User interrupted with Ctrl+C |
| 143 | Process terminated by SIGTERM | Process killed by timeout or system |

### Common Error Patterns

**Security-related errors**:
```
Command blocked by security validation
Directory access denied: /restricted/path
Command exceeds maximum length limit
Injection pattern detected in command
```

**Resource-related errors**:
```
Command timed out after X seconds
Output truncated due to size limit
Maximum line length exceeded
Process cleanup failed
```

**Environment-related errors**:
```
Working directory not found
Environment variable validation failed
Configuration file parsing error
Permission denied for configuration file
```

## Performance Optimization

### Command Optimization

**Use efficient commands**:
```bash
# Efficient file counting
sah shell "find . -type f | wc -l"

# Efficient disk usage
sah shell "du -sh ."

# Efficient process listing
sah shell "ps aux --sort=-%cpu | head -10"
```

**Avoid inefficient patterns**:
```bash
# Inefficient: loads all output into memory
sah shell "cat huge-file.log"

# Efficient: processes in chunks
sah shell "head -n 1000 huge-file.log"
sah shell "tail -n 1000 huge-file.log"
```

### Configuration Tuning

**Optimize for your environment**:
```toml
# Development: permissive settings
[shell.execution]
default_timeout = 600
max_timeout = 3600

[shell.output]
max_output_size = "50MB"

# Production: conservative settings  
[shell.execution]
default_timeout = 300
max_timeout = 900

[shell.output]
max_output_size = "1MB"
```

**Monitor and adjust**:
```bash
# Monitor actual usage patterns
grep "execution_time_ms" /var/log/sah-shell.log | awk '{print $NF}' | sort -n

# Monitor output sizes
grep "output_size" /var/log/sah-shell.log | awk '{print $NF}' | sort -n
```

## Getting Help

### Log Analysis

**Shell tool logs**:
```bash
# Check system logs
journalctl -u sah-shell

# Check application logs
tail -f /var/log/sah-shell.log

# Search for specific errors
grep -i "error\|failed\|timeout" /var/log/sah-shell.log
```

**Debug logging**:
```bash
# Enable debug logging
export RUST_LOG=debug
sah shell "test-command"

# Check debug output
grep "DEBUG" /var/log/sah-shell.log
```

### Community Resources

- **Documentation**: Refer to other sections of this guide
- **Issue Reporting**: Report bugs and issues to the project repository
- **Configuration Examples**: Check the [Examples](examples/) section
- **Security Guidelines**: Review the [Security](security.md) guide

### Professional Support

For enterprise environments:
1. Implement comprehensive monitoring
2. Regular security audits
3. Performance baseline establishment
4. Incident response procedures
5. Regular configuration reviews

## Prevention Strategies

### Proactive Monitoring

**Set up alerts for**:
- High command failure rates
- Unusual execution times
- Security validation failures
- Resource exhaustion events
- Configuration changes

**Regular health checks**:
```bash
# Daily health check script
#!/bin/bash
sah shell "echo 'Health check'" || echo "ALERT: Shell tool not responding"
sah shell -t 10 "date" || echo "ALERT: Timeout issues"
sah shell --format json "whoami" | jq '.is_error' || echo "ALERT: JSON parsing issues"
```

### Best Practices Implementation

1. **Regular Updates**: Keep shell tool and dependencies updated
2. **Configuration Reviews**: Regularly review and update security settings
3. **Documentation**: Maintain documentation of configuration changes
4. **Testing**: Test configuration changes in non-production environments
5. **Monitoring**: Implement comprehensive monitoring and alerting
6. **Training**: Ensure team members understand security implications
7. **Backup**: Maintain backups of working configurations

This troubleshooting guide should help resolve most common issues. For persistent problems or security concerns, consult the [Security Guide](security.md) or contact your system administrator.