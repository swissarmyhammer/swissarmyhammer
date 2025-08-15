# Shell Tool Security

The shell tool includes comprehensive security features designed to prevent malicious command execution while enabling legitimate automation tasks. This guide covers security mechanisms, best practices, and configuration for different environments.

## Security Architecture

The shell tool implements defense-in-depth security with multiple layers:

1. **Input Validation**: Commands are validated before execution
2. **Command Filtering**: Dangerous commands are blocked based on patterns  
3. **Access Controls**: Directory and resource access restrictions
4. **Process Isolation**: Commands execute in separate processes
5. **Resource Limits**: Timeout and output size controls prevent resource exhaustion
6. **Audit Logging**: Comprehensive logging for security monitoring

## Security Features

### Command Injection Prevention

The shell tool prevents common command injection attacks through multiple mechanisms:

**Input Sanitization**
- Command length limits prevent buffer overflow attempts
- Character validation blocks dangerous command sequences
- Pattern matching detects injection attempts

**Command Parsing**
- Commands are parsed and validated before execution
- Shell metacharacters are handled safely
- Environment variable expansion is controlled

**Example Blocked Patterns:**
```bash
# These patterns are automatically blocked:
"rm -rf /" && echo "safe"     # Command chaining
"ls; rm -rf /home"            # Command separation
"cat /etc/passwd | mail..."   # Data exfiltration attempts
"$(dangerous_command)"        # Command substitution
"`dangerous_command`"         # Backtick expansion
```

### Access Control System

**Directory Restrictions**
When configured, commands can only execute in allowed directories:

```toml
[shell.security]
allowed_directories = [
    "/home/user/projects",    # User project directory
    "/tmp",                   # Temporary files
    "/opt/safe-workspace"     # Designated safe area
]
```

**Command Blacklisting**
Dangerous commands are blocked by default:

```toml
[shell.security]
blocked_commands = [
    "rm -rf /",              # Prevent root deletion
    "format",                # Block disk formatting
    "dd if=",                # Dangerous disk operations
    "mkfs",                  # Filesystem creation
    "fdisk",                 # Disk partitioning
    "sudo rm",               # Privileged deletions
    "> /dev/"                # Device overwriting
]
```

### Process Isolation

**Resource Containment**
- Commands execute in separate processes
- Child process cleanup prevents orphaned processes
- Memory and file descriptor limits apply per process
- Process tree termination on timeout

**Environment Control**
- Environment variables are isolated per execution
- Sensitive environment variables can be filtered
- Working directory restrictions limit file access
- Process permissions inherit from the parent safely

### Resource Protection

**Timeout Controls**
```toml
[shell.execution]
default_timeout = 300        # 5 minutes default
max_timeout = 1800          # 30 minutes maximum
min_timeout = 1             # 1 second minimum
```

**Output Limits**
```toml
[shell.output]
max_output_size = "10MB"    # Prevent memory exhaustion
max_line_length = 2000      # Line length limits
```

**Process Management**
- Automatic cleanup of child processes
- Resource monitoring and limits
- Graceful termination with SIGTERM, then SIGKILL

## Security Best Practices

### For Development Environments

**Moderate Security Settings**
```toml
[shell.security]
enable_validation = true
blocked_commands = [
    "rm -rf /", "format", "dd if=", "mkfs", "fdisk"
]
allowed_directories = ["/home/dev", "/tmp", "/opt/projects"]
max_command_length = 1500
enable_injection_detection = true
```

**Safe Development Practices**
- Use relative paths within project directories
- Avoid commands that modify system configuration
- Test commands with shorter timeouts during development
- Use version control for all script changes

**Example Safe Development Commands**
```bash
# Build and test operations
sah shell -C /project "cargo build"
sah shell -C /project "npm test"
sah shell -C /project "git status"

# File operations within project
sah shell -C /project "find . -name '*.rs' -exec grep -l 'TODO' {} \;"
sah shell -C /project "wc -l src/**/*.rs"
```

### For Production Environments

**Strict Security Configuration**
```toml
[shell.security]
enable_validation = true
blocked_commands = [
    "rm", "rmdir", "del", "format", "fdisk", "mkfs", "dd",
    "sudo", "su", "chmod 777", "chown", "mount", "umount",
    "iptables", "ufw", "systemctl", "service"
]
allowed_directories = ["/app", "/tmp/app"]
max_command_length = 500
enable_injection_detection = true
```

**Production Security Practices**
- Enable comprehensive audit logging
- Monitor security events regularly
- Use minimal necessary permissions
- Implement command whitelisting for critical systems
- Regular security reviews and updates

**Example Production Commands**
```bash
# Application health checks
sah shell -C /app "curl -f http://localhost:8080/health"
sah shell -C /app "ps aux | grep 'myapp' | grep -v grep"

# Log analysis (safe operations)
sah shell -C /app/logs "tail -n 100 application.log"
sah shell -C /app "df -h /app"
```

### For CI/CD Environments

**Balanced Security for Automation**
```toml
[shell.security]
enable_validation = true
blocked_commands = ["rm -rf /", "format", "fdisk", "sudo"]
allowed_directories = ["/builds", "/cache", "/tmp", "/opt/ci"]
max_command_length = 1000
enable_injection_detection = true
```

**CI/CD Security Practices**
- Use dedicated CI user accounts
- Isolate build environments
- Validate all build scripts before execution
- Monitor for unusual build activity
- Implement secrets management for sensitive data

**Example CI/CD Commands**
```bash
# Build operations
sah shell -C /builds/project "npm ci && npm run build"
sah shell -C /builds/project "cargo test --release"

# Deployment checks
sah shell "docker ps | grep myapp"
sah shell "kubectl get pods -n production"
```

## Security Configuration Examples

### High Security Configuration
For security-sensitive environments:

```toml
[shell.security]
enable_validation = true
blocked_commands = [
    # File system operations
    "rm", "rmdir", "del", "mv", "cp /etc", "cp /usr", "cp /var",
    
    # Disk operations
    "format", "fdisk", "mkfs", "fsck", "dd",
    
    # System administration
    "sudo", "su", "passwd", "useradd", "userdel", "usermod",
    "mount", "umount", "chroot", "systemctl", "service",
    
    # Network operations
    "iptables", "ufw", "netstat", "ss", "tcpdump",
    
    # Package management
    "apt", "yum", "dnf", "zypper", "pkg",
    
    # Dangerous patterns
    "> /dev/", "< /dev/", "2> /dev/null; rm",
    "&& rm", "; rm", "| rm", "$(", "`"
]
allowed_directories = ["/app", "/tmp/app"]
max_command_length = 300
enable_injection_detection = true

[shell.audit]
enable_audit_logging = true
log_level = "info"  
log_command_output = false
max_audit_entry_size = 5000
```

### Permissive Development Configuration
For developer productivity:

```toml
[shell.security]
enable_validation = true
blocked_commands = [
    "rm -rf /", "format", "dd if=/dev/zero", "mkfs", "> /dev/sda"
]
allowed_directories = [
    "/home/dev", "/tmp", "/opt", "/var/tmp",
    "/usr/local/src", "/workspace"
]
max_command_length = 2000
enable_injection_detection = true

[shell.audit]
enable_audit_logging = false  # Reduced noise for development
log_level = "warn"
log_command_output = false
max_audit_entry_size = 10000
```

## Threat Model and Mitigations

### Command Injection Attacks

**Threat**: Attackers attempt to execute arbitrary commands through command injection.

**Mitigations**:
- Input validation and sanitization
- Command pattern blocking  
- Shell metacharacter filtering
- Parameterized command construction

**Detection**:
```toml
[shell.security]
enable_injection_detection = true
blocked_commands = [
    "$(", "`", ";", "&&", "||", "|", ">", "<", "&"
]
```

### Privilege Escalation

**Threat**: Commands attempt to gain elevated privileges.

**Mitigations**:
- Block privilege escalation commands (sudo, su, etc.)
- Run with minimal necessary permissions
- Directory access restrictions
- Process isolation

**Prevention**:
```toml
[shell.security]  
blocked_commands = [
    "sudo", "su", "passwd", "chsh", "chfn",
    "chmod 777", "chmod +s", "chown root"
]
```

### Resource Exhaustion

**Threat**: Commands consume excessive resources (CPU, memory, disk).

**Mitigations**:
- Timeout controls prevent runaway processes
- Output size limits prevent memory exhaustion
- Process cleanup prevents orphaned processes
- Resource monitoring and alerting

**Protection**:
```toml
[shell.execution]
default_timeout = 300
max_timeout = 1800
cleanup_process_tree = true

[shell.output]
max_output_size = "10MB"
max_line_length = 2000
```

### Data Exfiltration

**Threat**: Commands attempt to read and exfiltrate sensitive data.

**Mitigations**:
- Directory access restrictions
- Network command blocking
- File access monitoring
- Audit logging of all commands

**Prevention**:
```toml
[shell.security]
allowed_directories = ["/safe/workspace"]
blocked_commands = [
    "curl", "wget", "nc", "netcat", "ssh", "scp", "rsync",
    "mail", "sendmail", "cat /etc/passwd", "cat /etc/shadow"
]
```

## Incident Response

### Security Event Detection

**Monitor for**:
- Commands blocked by security validation
- Attempts to access restricted directories
- Unusual command patterns or lengths
- Repeated failed execution attempts
- Commands with suspicious arguments

**Audit Log Analysis**:
```bash
# Review blocked commands
grep "BLOCKED" /var/log/sah-shell.log

# Check access violations
grep "DIRECTORY_DENIED" /var/log/sah-shell.log

# Analyze command patterns
awk '{print $NF}' /var/log/sah-shell.log | sort | uniq -c | sort -nr
```

### Response Procedures

1. **Immediate Response**:
   - Block suspicious IP addresses if external
   - Increase monitoring and logging levels
   - Review recent command executions
   - Check system integrity

2. **Investigation**:
   - Analyze audit logs for attack patterns
   - Check system logs for related events
   - Verify system configuration integrity
   - Review user account activity

3. **Remediation**:
   - Update command blocking rules
   - Strengthen directory restrictions
   - Implement additional monitoring
   - Update security configurations

### Security Monitoring

**Key Metrics to Monitor**:
- Number of blocked commands per hour/day
- Average command execution time
- Failed execution attempts by user/source
- Unusual command patterns or arguments
- Resource usage during command execution

**Alerting Thresholds**:
- More than 10 blocked commands in 1 hour
- Commands exceeding 50% of max timeout
- Access attempts to restricted directories
- Commands with lengths near the maximum limit
- Repeated failures from the same source

## Compliance and Auditing

### Audit Logging

**Enable Comprehensive Logging**:
```toml
[shell.audit]
enable_audit_logging = true
log_level = "info"
log_command_output = false  # Security: avoid logging sensitive data
max_audit_entry_size = 10000
```

**Audit Log Contents**:
- Timestamp of execution
- Command executed (sanitized)
- User/context information
- Execution result (success/failure)
- Resource usage metrics
- Security validation results

### Compliance Requirements

**For SOX Compliance**:
- Enable audit logging
- Implement command approval workflows
- Regular security configuration reviews
- Access control documentation

**For GDPR Compliance**:
- Avoid logging personal data in commands
- Implement data retention policies
- Provide audit log access controls
- Document data processing activities

**For HIPAA Compliance**:
- Encrypt audit logs
- Implement strict access controls
- Regular security assessments
- Incident response procedures

## Security Testing

### Penetration Testing

**Test Command Injection**:
```bash
# These should be blocked by security validation
sah shell "echo test; rm -rf /tmp/testfile"
sah shell "ls $(whoami)"
sah shell "cat /etc/passwd | head -1"
```

**Test Access Controls**:
```bash
# These should be blocked if directory restrictions are enabled
sah shell -C /etc "ls -la"
sah shell -C /root "pwd"
sah shell -C /home/other-user "ls"
```

**Test Resource Limits**:
```bash
# Test timeout controls
sah shell -t 5 "sleep 10"

# Test output limits
sah shell "yes | head -n 100000"

# Test command length limits
sah shell "$(python -c 'print("echo " + "a" * 2000)')"
```

### Vulnerability Assessment

**Regular Security Checks**:
1. Review and update blocked command patterns
2. Test directory access restrictions
3. Validate timeout and resource limits
4. Check audit logging functionality
5. Verify process cleanup mechanisms

**Security Scan Checklist**:
- [ ] Command injection patterns blocked
- [ ] Directory restrictions enforced
- [ ] Resource limits effective
- [ ] Audit logging functional
- [ ] Process cleanup working
- [ ] Security configurations current
- [ ] Access controls validated

## Best Practices Summary

### Security Configuration
1. Enable all security validation features
2. Use conservative command blocking lists
3. Implement directory access restrictions
4. Set appropriate resource limits
5. Enable comprehensive audit logging

### Operational Security
1. Regular security configuration reviews
2. Monitor audit logs for suspicious activity
3. Keep security patterns updated
4. Test security controls regularly
5. Implement incident response procedures

### Development Security
1. Test commands in safe environments first
2. Use minimal necessary permissions
3. Avoid hardcoding sensitive data
4. Regular security training for developers
5. Code review for shell command usage

For additional security guidance, see:
- [Configuration Reference](configuration.md) - Security configuration options
- [Troubleshooting Guide](troubleshooting.md) - Security-related issues
- [Examples](examples/) - Secure usage patterns