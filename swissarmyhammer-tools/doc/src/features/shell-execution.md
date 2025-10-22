# Shell Execution

SwissArmyHammer provides shell command execution with environment control and proper output handling for interactive AI workflows.

## Overview

The shell execution tool enables AI assistants to run commands in your development environment, such as builds, tests, linters, and other development tools.

## Available Tool

### shell_execute

Execute shell commands with proper output handling.

**Parameters:**
- `command` (required): The shell command to execute
- `working_directory` (optional): Working directory for command execution (default: current directory)
- `environment` (optional): Additional environment variables as JSON string

**Example:**
```json
{
  "command": "cargo test",
  "working_directory": "/project/path"
}
```

**Returns:**
- Command output (stdout/stderr)
- Exit code
- Execution time
- Working directory

## Use Cases

### Running Tests

Execute test suites:

```json
{
  "command": "cargo test"
}
```

```json
{
  "command": "npm test"
}
```

```json
{
  "command": "pytest"
}
```

### Building Projects

Run build commands:

```json
{
  "command": "cargo build --release"
}
```

```json
{
  "command": "npm run build"
}
```

```json
{
  "command": "make"
}
```

### Linting Code

Run linters and formatters:

```json
{
  "command": "cargo clippy"
}
```

```json
{
  "command": "eslint src/"
}
```

```json
{
  "command": "black ."
}
```

### Git Operations

Execute git commands:

```json
{
  "command": "git status"
}
```

```json
{
  "command": "git diff"
}
```

```json
{
  "command": "git log --oneline -10"
}
```

### Package Management

Manage dependencies:

```json
{
  "command": "cargo add tokio"
}
```

```json
{
  "command": "npm install axios"
}
```

```json
{
  "command": "pip install requests"
}
```

## Environment Variables

### Adding Variables

Pass environment variables as JSON:

```json
{
  "command": "cargo build",
  "environment": "{\"RUST_LOG\":\"debug\",\"CARGO_INCREMENTAL\":\"0\"}"
}
```

### Common Variables

**Rust:**
```json
{
  "environment": "{\"RUST_LOG\":\"debug\",\"RUSTFLAGS\":\"-D warnings\"}"
}
```

**Node.js:**
```json
{
  "environment": "{\"NODE_ENV\":\"production\",\"DEBUG\":\"*\"}"
}
```

**Python:**
```json
{
  "environment": "{\"PYTHONPATH\":\".\",\"DEBUG\":\"1\"}"
}
```

## Working Directory

### Specifying Directory

Run commands in specific directories:

```json
{
  "command": "cargo test",
  "working_directory": "/Users/dev/project"
}
```

### Relative Paths

Working directory must be absolute. Use full paths:

```json
{
  "command": "npm test",
  "working_directory": "/home/user/workspace/frontend"
}
```

## Output Handling

### Standard Output

Commands capture both stdout and stderr:

```json
{
  "command": "cargo build"
}
```

Returns:
```
Compiling project v0.1.0
Finished dev [unoptimized + debuginfo] target(s) in 2.34s
```

### Exit Codes

Non-zero exit codes indicate failure:

```json
{
  "command": "cargo test"
}
```

Returns:
- Exit code: 0 (success)
- Exit code: 1 (failure)

### Execution Time

Response includes execution time:
```
Command completed in 2.34s
```

## Integration Patterns

### Test-Driven Development

1. Write test: `files_write`
2. Run test: `shell_execute cargo test`
3. See failure
4. Implement: `files_edit`
5. Run test: `shell_execute cargo test`
6. See success

### Build and Check

1. Make changes: `files_edit`
2. Build: `shell_execute cargo build`
3. Check errors
4. Fix issues
5. Repeat

### Lint and Fix

1. Run linter: `shell_execute cargo clippy`
2. Review warnings
3. Fix issues: `files_edit`
4. Run again: `shell_execute cargo clippy`
5. Verify clean

### Git Workflow

1. Check status: `shell_execute git status`
2. Make changes: `files_edit`
3. Check diff: `shell_execute git diff`
4. Stage: `shell_execute git add .`
5. Commit: `shell_execute git commit -m "message"`

## Best Practices

### Command Selection

1. **Use Simple Commands**: Avoid complex shell scripting
2. **No Interactive Commands**: Don't use commands requiring input
3. **Check Exit Codes**: Always review exit code
4. **Read Output**: Review output for errors

### Security

1. **Validate Commands**: Don't execute arbitrary user input
2. **Limit Scope**: Use working directory to restrict access
3. **No Destructive Commands**: Avoid rm -rf or similar
4. **Review First**: Understand what command does

### Error Handling

1. **Check Exit Code**: Non-zero means failure
2. **Read Errors**: stderr contains error messages
3. **Retry Logic**: Some commands can be retried
4. **Graceful Failure**: Handle command failures

### Performance

1. **Avoid Long Commands**: Some commands take time
2. **Use Appropriate Timeouts**: Don't wait forever
3. **Stream Output**: Large output may be truncated
4. **Parallel Execution**: Run independent commands in parallel

## Common Commands

### Rust Development

```json
{"command": "cargo build"}
{"command": "cargo test"}
{"command": "cargo clippy"}
{"command": "cargo fmt"}
{"command": "cargo run"}
```

### Node.js Development

```json
{"command": "npm install"}
{"command": "npm test"}
{"command": "npm run build"}
{"command": "npx eslint ."}
{"command": "npx prettier --write ."}
```

### Python Development

```json
{"command": "pip install -r requirements.txt"}
{"command": "pytest"}
{"command": "black ."}
{"command": "mypy ."}
{"command": "python -m module"}
```

### Git Commands

```json
{"command": "git status"}
{"command": "git diff"}
{"command": "git log --oneline"}
{"command": "git branch"}
{"command": "git add ."}
```

## Limitations

### No Interactive Input

Commands requiring interactive input won't work:
- `git rebase -i` (interactive rebase)
- `npm init` (without -y)
- `vim` (interactive editor)

Use non-interactive alternatives when available.

### No Long-Running Processes

Don't use for:
- Starting servers
- Running daemons
- Watching file systems

These will block until killed.

### Output Size Limits

Very large output may be truncated. For large output:
- Redirect to file
- Use pagination
- Limit output with command flags

### Shell Features

Limited shell features:
- No pipes (use command flags instead)
- No redirects (captured automatically)
- No job control
- No aliases

## Troubleshooting

### Command Not Found

**Issue:** Command not found error.

**Solution:**
- Verify command is installed
- Check PATH includes command
- Use full path to command
- Install missing tool

### Permission Denied

**Issue:** Permission denied error.

**Solution:**
- Check file permissions
- Verify working directory access
- Don't use sudo (security risk)
- Fix permissions before running

### Command Hangs

**Issue:** Command doesn't complete.

**Solution:**
- Command may be interactive
- Command may be waiting for input
- Command may be long-running
- Use non-interactive version

### Wrong Directory

**Issue:** Command runs in wrong directory.

**Solution:**
- Specify working_directory explicitly
- Use absolute paths
- Verify directory exists
- Check permissions

## Security Considerations

### Command Injection

Never execute untrusted user input:
- Validate commands before execution
- Use parameterized commands when possible
- Avoid shell metacharacters
- Sanitize inputs

### Destructive Commands

Be careful with:
- `rm -rf`
- `git push --force`
- `cargo clean` (loses build artifacts)
- File deletion commands

### Sensitive Data

Don't pass sensitive data:
- Passwords
- API keys
- Secrets
- Credentials

Use environment variables or config files.

## Next Steps

- [Git Integration](./git-integration.md): Track changes
- [File Operations](./file-operations.md): Read and write files
- [Issue Management](./issue-management.md): Track work
