Execute shell commands with proper output handling for interactive AI workflows.

## Parameters

- `command` (required): The shell command to execute
- `working_directory` (optional): Working directory for command execution (default: current directory)
- `environment` (optional): Additional environment variables as JSON string

## Examples

```json
{
  "command": "cargo test",
  "working_directory": "/project/path"
}
```

## Returns

Returns command output (stdout/stderr), exit code, execution time, and working directory.
