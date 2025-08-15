# Core Shell Execution Engine Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Implement the core shell command execution engine with proper process management, timeout controls, and output capture following the specification requirements.

## Objective

Build the fundamental command execution functionality using `std::process::Command` with comprehensive output capture, exit code handling, and basic error management.

## Requirements

### Command Execution Core
- Use `std::process::Command` for process spawning
- Capture both stdout and stderr streams
- Preserve and return exit codes
- Handle command execution in async context using tokio

### Output Management
- Capture complete stdout and stderr output
- Handle both text and binary output gracefully
- Preserve output ordering and timing
- Return structured output in the response

### Error Handling
- Handle process spawn failures
- Distinguish between execution errors and command failures
- Provide detailed error messages with context
- Return appropriate error responses via MCP protocol

### Basic Response Structure
Implement the response format from specification:
- Successful execution with metadata
- Command failures with exit codes
- Basic error conditions
- Execution timing information

## Implementation Details

### Core Function Signature
```rust
pub async fn execute_shell_command(
    command: String,
    working_directory: Option<PathBuf>,
    timeout_seconds: u64,
    environment: Option<HashMap<String, String>>,
) -> Result<ShellExecutionResult, ShellError>
```

### Response Structure
```rust
pub struct ShellExecutionResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub execution_time_ms: u64,
    pub working_directory: PathBuf,
}
```

### Error Types
Create comprehensive error types:
- `CommandSpawnError`: Failed to start process
- `ExecutionError`: Runtime execution failures
- `InvalidCommand`: Command validation failures
- `SystemError`: System-level issues

## Architecture Patterns

### Async Integration
- Use tokio for async process execution
- Ensure proper async context throughout
- Handle async cancellation gracefully
- Follow existing async patterns in the codebase

### Error Propagation
- Use the established `Result<T, E>` patterns
- Integrate with existing error hierarchy
- Provide rich error context and details
- Follow the error handling patterns from other tools

## Acceptance Criteria

- [ ] Basic command execution works with std::process::Command
- [ ] Stdout and stderr captured correctly
- [ ] Exit codes preserved and returned
- [ ] Async execution integrated properly
- [ ] Error handling comprehensive and informative
- [ ] Response structure matches specification format
- [ ] Integration tests pass with basic commands
- [ ] Memory usage reasonable for command output

## Notes

- This step focuses on the core execution without timeout or security features
- Working directory and environment variable support included but basic
- Timeout will be added in the next step for proper process management
- Security validation will be handled in a separate step
- Focus on correctness and reliability of basic execution