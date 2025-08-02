# Shell Action Specification for Workflows

## Overview

This specification defines a new workflow action type called `shell` that allows workflows to execute shell commands directly. This feature will enable workflows to interact with the system, run build scripts, perform file operations, and integrate with external tools.

## Syntax

The shell action follows the existing workflow action syntax patterns:

Make sure `shell` is case insensitive.

```
Shell "command to execute"
Shell "command" with timeout=30
Shell "command" with timeout=30 result="output_variable" 
```

### Basic Format
- `Shell "command"` - Execute a shell command with default settings
- Case-insensitive: `shell "command"` also works

### Extended Format with Parameters
- `Shell "command" with timeout=N` - Execute with custom timeout (in seconds)
- `Shell "command" with result="variable_name"` - Store output in a variable
- `Shell "command" with timeout=30 result="output"` - Combine timeout and result capture

## Parameters

### Required Parameters
- `command` (string): The shell command to execute, provided as a quoted string

### Optional Parameters
- `timeout` (integer): Maximum execution time in seconds (default: 30)
- `result` (string): Variable name to store the command output
- `working_dir` (string): Working directory for command execution (default: current directory)
- `env` (object): Environment variables to set for the command

## Behavior

### Command Execution
- Commands are executed in a subprocess using the system shell
- Standard output and standard error are captured
- Exit codes are monitored and affect workflow transitions
- Commands run with the same user permissions as the workflow executor

### Variable Setting
When execution completes, the following variables are automatically set:
- `success`: Boolean indicating if the command succeeded (exit code 0)
- `failure`: Boolean indicating if the command failed (exit code != 0)
- `exit_code`: Integer exit code from the command
- `stdout`: Standard output from the command
- `stderr`: Standard error from the command
- `duration_ms`: Execution time in milliseconds
- `result`: Command output (if `result` parameter specified)

### Timeout Handling
- Commands that exceed the timeout are terminated
- Timeout triggers a failure state (success=false, failure=true)
- Default timeout is no timeout at all
- There is no maximum timeout

## Security Considerations

### Command Injection Prevention
- Commands are executed through proper shell escaping
- No variable substitution occurs within the command string itself
- User input must be sanitized before being used in shell commands

### Restricted Operations
- Commands cannot modify the workflow execution environment
- No access to workflow internal state beyond defined variables
- Network operations should be carefully considered

### Execution Limits
- Maximum execution time: 300 seconds
- Memory usage monitoring (implementation-dependent)
- Process isolation from workflow executor

### Dangerous Commands
The following types of commands should trigger warnings or restrictions:
- Commands that modify system configuration
- Commands that install software
- Commands with elevated privileges
- Commands that access sensitive directories

## Usage Examples

### Basic Command Execution
```yaml
Actions:
- BuildProject: Shell "cargo build --release"
- TestProject: Shell "cargo test"
- CheckVersion: Shell "git describe --tags" with result="version"
```

### With Timeout and Result Capture
```yaml
Actions:
- LongRunning: Shell "npm run build" with timeout=120 result="build_output"
- QuickCheck: Shell "ls -la" with timeout=5 result="file_list"
```

### Conditional Execution Based on Results
```yaml
Actions:
- CheckGit: Shell "git status --porcelain" with result="git_status"
- HandleClean: Log "Repository is clean"
- HandleDirty: Log "Repository has uncommitted changes"

# Transitions based on shell command results
CheckGit --> HandleClean: git_status == ""
CheckGit --> HandleDirty: git_status != ""
```

### Environment and Working Directory
```yaml
Actions:
- CustomEnv: Shell "echo $CUSTOM_VAR" with env={"CUSTOM_VAR": "hello"} result="greeting"
- InSubdir: Shell "ls" with working_dir="./src" result="source_files"
```

## Integration with Existing Actions

### Variable Substitution
Shell commands can use variables from previous workflow steps:
```yaml
Actions:
- SetFile: Set filename="test.txt"
- ProcessFile: Shell "cat ${filename}" with result="file_contents"
```

### Chaining with Other Actions
```yaml
Actions:
- GetCommit: Shell "git rev-parse HEAD" with result="commit_hash"
- AnalyzeCommit: Execute prompt "analyze-commit" with commit="${commit_hash}"
```

## Error Handling

### Command Failures
- Non-zero exit codes set `failure=true` and `success=false`
- Workflows can branch on failure conditions
- Error output is captured in `stderr` variable

### Timeout Scenarios
- Processes exceeding timeout are terminated with SIGTERM
- If process doesn't respond, SIGKILL is used after grace period
- Timeout failures set appropriate error variables

### System Errors
- Permission denied, command not found, etc. are treated as failures
- System error details are included in `stderr` variable

## Implementation Notes

### Parser Integration
- Add shell action parsing to `ActionParser` in `action_parser.rs`
- Follow existing patterns for command parsing and validation
- Support case-insensitive command recognition

### Execution Infrastructure
- Create `ShellAction` struct in `actions.rs`
- Implement proper subprocess management with tokio
- Add timeout handling and process cleanup

### Security Implementation
- Implement command validation and sanitization
- Add configurable restrictions for dangerous operations
- Provide audit logging for shell command execution

## Future Enhancements

### Advanced Features
- Support for command pipelines
- Interactive command support with input/output streams
- Parallel command execution
- Command templates with parameter substitution

### Security Enhancements
- Sandboxed execution environments
- Command allowlists and blocklists
- Resource usage monitoring and limits

### Integration Features
- Integration with workflow logging system
- Command history and audit trails
- Performance metrics and monitoring

## Compatibility

This specification is designed to be backward compatible with existing workflows. The new shell action type will not interfere with existing action types and follows established patterns for syntax and behavior.