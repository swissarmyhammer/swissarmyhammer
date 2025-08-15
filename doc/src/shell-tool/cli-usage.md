# CLI Usage Guide

The `sah shell` command provides direct access to shell execution functionality with comprehensive options for controlling execution environment, output format, and security settings.

## Command Syntax

```bash
sah shell [OPTIONS] <COMMAND>
```

### Basic Examples
```bash
# Execute a simple command
sah shell "echo 'Hello, World!'"

# Run command in specific directory
sah shell -C /project "ls -la"

# Set timeout and environment variables
sah shell -t 300 -e "DEBUG=1" -e "NODE_ENV=production" "npm test"
```

## Command Options

### `-C, --directory <DIR>`
Set the working directory for command execution.

**Examples:**
```bash
# Run tests in project directory
sah shell -C /home/user/project "cargo test"

# Check logs in system directory
sah shell -C /var/log "tail -n 20 application.log"

# Build from source directory
sah shell --directory /opt/build "./configure && make"
```

### `-t, --timeout <SECONDS>`
Set command timeout in seconds (default: 300, max: 1800).

**Examples:**
```bash
# Short timeout for quick commands
sah shell -t 30 "ping -c 3 google.com"

# Long timeout for build processes
sah shell -t 1800 "cargo build --release"

# Medium timeout for test suites
sah shell --timeout 600 "npm test"
```

### `-e, --env <KEY=VALUE>`
Set environment variables (can be used multiple times).

**Examples:**
```bash
# Single environment variable
sah shell -e "RUST_LOG=debug" "cargo run"

# Multiple environment variables
sah shell -e "NODE_ENV=production" -e "PORT=8080" -e "DEBUG=false" "node server.js"

# Build with custom environment
sah shell -e "CC=gcc" -e "CFLAGS=-O2" "./configure"
```

### `--format <FORMAT>`
Set output format: `human` (default), `json`, or `yaml`.

**Examples:**
```bash
# Human-readable output (default)
sah shell "date"

# JSON output for scripting
sah shell --format json "whoami" | jq '.metadata.stdout'

# YAML output for readability
sah shell --format yaml "uname -a"
```

### `--show-metadata`
Include execution metadata in output.

**Examples:**
```bash
# Show execution timing and details
sah shell --show-metadata "sleep 2"

# View exit codes and timing
sah shell --show-metadata "ls /nonexistent || true"

# Debug command execution
sah shell --show-metadata -t 5 "echo 'test' && sleep 1"
```

### `-q, --quiet`
Suppress command output, showing only execution results.

**Examples:**
```bash
# Silent execution with status only
sah shell -q "make clean"

# Check command success without output
sah shell --quiet "test -f important-file.txt"

# Silent build with result only
sah shell -q --show-metadata "cargo build"
```

## Usage Patterns

### Development Workflows

**Running Tests:**
```bash
# Basic test execution
sah shell -C /project "cargo test"

# Tests with debug output
sah shell -C /project -e "RUST_LOG=debug" -e "RUST_BACKTRACE=1" "cargo test"

# Tests with timeout and metadata
sah shell -t 900 --show-metadata -C /project "cargo test --release"
```

**Building Projects:**
```bash
# Debug build
sah shell -C /project "cargo build"

# Release build with timing
sah shell -t 1200 --show-metadata -C /project "cargo build --release"

# Build with custom environment
sah shell -C /project -e "RUSTFLAGS='-C target-cpu=native'" "cargo build --release"
```

**Git Operations:**
```bash
# Check repository status
sah shell -C /project "git status --porcelain"

# View commit history
sah shell -C /project "git log --oneline -n 10"

# Check for uncommitted changes
sah shell --quiet -C /project "git diff --exit-code"
```

### System Administration

**System Monitoring:**
```bash
# System resource usage
sah shell --format json "df -h && free -h && uptime"

# Process monitoring
sah shell -t 60 "ps aux | sort -k 3 -nr | head -10"

# Service status checking
sah shell "systemctl status nginx"
```

**File System Operations:**
```bash
# Find large files
sah shell -t 300 "find /var -type f -size +100M -ls"

# Directory usage analysis
sah shell "du -sh /home/* | sort -rh | head -10"

# Log file monitoring
sah shell -t 60 -C /var/log "tail -f application.log"
```

**Network Diagnostics:**
```bash
# Network connectivity
sah shell -t 30 "ping -c 5 8.8.8.8"

# Port scanning
sah shell -t 60 "nmap -p 80,443,22 localhost"

# Network interface status
sah shell "ip addr show"
```

### CI/CD Integration

**Automated Testing:**
```bash
# Run full test suite
sah shell -t 1800 -e "CI=true" -e "NODE_ENV=test" "npm run test:ci"

# Integration tests
sah shell -t 900 -C /project/tests "docker-compose up -d && npm test && docker-compose down"

# Security scans
sah shell -t 600 "npm audit --audit-level=moderate"
```

**Build and Deploy:**
```bash
# Production build
sah shell -t 1200 -e "NODE_ENV=production" -e "BUILD_TARGET=production" "npm run build"

# Docker image build
sah shell -t 1800 "docker build -t myapp:latest ."

# Deployment checks
sah shell -t 300 "kubectl get pods -n production"
```

## Output Examples

### Human Format (Default)
```bash
$ sah shell "echo 'Hello' && date"
Hello
Wed Aug 15 10:30:45 PDT 2025

Command executed successfully (exit code: 0)
Execution time: 25ms
```

### JSON Format
```bash
$ sah shell --format json "date"
{
  "content": [
    {
      "type": "text",
      "text": "Command executed successfully"
    }
  ],
  "is_error": false,
  "metadata": {
    "command": "date",
    "exit_code": 0,
    "stdout": "Wed Aug 15 10:30:45 PDT 2025\n",
    "stderr": "",
    "execution_time_ms": 15,
    "working_directory": "/Users/user/project"
  }
}
```

### YAML Format
```bash
$ sah shell --format yaml "whoami"
content:
  - type: text
    text: Command executed successfully
is_error: false
metadata:
  command: whoami
  exit_code: 0
  stdout: |
    user
  stderr: ""
  execution_time_ms: 12
  working_directory: /Users/user/project
```

## Error Handling

### Command Failures
```bash
$ sah shell "ls /nonexistent"
ls: /nonexistent: No such file or directory

Command failed with exit code 2
Execution time: 18ms
```

### Timeout Errors
```bash
$ sah shell -t 5 "sleep 10"
Command timed out after 5 seconds
Partial output may be available in metadata
```

### Permission Errors
```bash
$ sah shell "cat /etc/shadow"
cat: /etc/shadow: Permission denied

Command failed with exit code 1
Execution time: 8ms
```

## Advanced Usage

### Complex Command Chains
```bash
# Pipeline commands
sah shell "find . -name '*.rs' | xargs wc -l | sort -nr | head -10"

# Conditional execution
sah shell "test -f Cargo.toml && cargo check || echo 'Not a Rust project'"

# Background processes (with timeout)
sah shell -t 60 "nohup ./long-running-service.sh > service.log 2>&1 & echo 'Started'"
```

### Environment Variable Management
```bash
# Load from environment file
sah shell -e "$(cat .env | tr '\n' ' ')" "npm start"

# Complex environment setup
sah shell \
  -e "PATH=/opt/custom/bin:$PATH" \
  -e "LD_LIBRARY_PATH=/opt/custom/lib" \
  -e "PKG_CONFIG_PATH=/opt/custom/lib/pkgconfig" \
  "./custom-build.sh"
```

### Output Processing
```bash
# Extract specific data with jq
sah shell --format json "ps aux" | jq -r '.metadata.stdout' | grep "node"

# Save execution metadata
sah shell --format json --show-metadata "build.sh" > build-report.json

# Monitor execution timing
sah shell --show-metadata "time-sensitive-command" | grep "execution_time_ms"
```

## Best Practices

### Security
- Use absolute paths for critical commands
- Set appropriate timeouts for expected execution duration
- Use working directory restrictions when possible
- Avoid passing sensitive data as command line arguments

### Performance
- Set realistic timeouts based on expected execution time
- Use quiet mode for scripting to reduce output processing
- Consider JSON format for programmatic processing
- Monitor execution time for performance optimization

### Reliability
- Test commands with shorter timeouts during development
- Use error checking for critical operations
- Implement retry logic for transient failures
- Log execution metadata for debugging

## Troubleshooting

Common issues and solutions:

**Command Not Found:**
- Verify command is in PATH or use absolute paths
- Check working directory settings
- Confirm command spelling and syntax

**Permission Denied:**
- Verify file permissions and ownership
- Check SELinux or similar security policies
- Ensure user has necessary privileges

**Timeout Issues:**
- Increase timeout for long-running operations
- Break complex operations into smaller steps
- Monitor system resource usage during execution

**Environment Issues:**
- Verify environment variable names and values
- Check for conflicting environment settings
- Use absolute paths in environment variables

For more detailed troubleshooting, see the [Troubleshooting Guide](troubleshooting.md).