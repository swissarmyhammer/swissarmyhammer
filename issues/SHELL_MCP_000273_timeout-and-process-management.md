# Timeout and Process Management Implementation  

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Add comprehensive timeout management and process cleanup functionality to the shell execution engine, including process tree termination and timeout error handling.

## Objective

Implement robust timeout controls using `tokio::time::timeout` and integrate with the existing `ProcessGuard` pattern for automatic process cleanup and orphan prevention.

## Requirements

### Timeout Implementation
- Use `tokio::time::timeout` for async timeout control
- Default timeout: 5 minutes (300 seconds)
- Maximum timeout: 30 minutes (1800 seconds) 
- Minimum timeout: 1 second
- Configurable timeout per execution

### Process Management
- Integrate with existing `ProcessGuard` pattern from workflow system
- Kill process tree on timeout to prevent orphaned processes
- Handle process cleanup on cancellation
- Proper signal handling for process termination

### Timeout Error Handling
- Return timeout-specific error responses
- Include partial output if available
- Preserve execution context in timeout errors
- Follow specification timeout response format

### Process Tree Management
- Kill child processes on timeout
- Handle process group termination
- Cross-platform process cleanup (Unix/Windows)
- Prevent zombie processes

## Implementation Details

### Timeout Integration
```rust
use tokio::time::{timeout, Duration};

pub async fn execute_with_timeout(
    command: String,
    timeout_seconds: u64,
    // ... other parameters
) -> Result<ShellExecutionResult, ShellError> {
    let timeout_duration = Duration::from_secs(timeout_seconds);
    
    match timeout(timeout_duration, execute_command_internal(command)).await {
        Ok(result) => result,
        Err(_) => handle_timeout_error(/* context */),
    }
}
```

### ProcessGuard Integration
- Leverage existing `ProcessGuard` from `swissarmyhammer/src/test_utils.rs`
- Ensure automatic cleanup on timeout or cancellation
- Handle process group management properly
- Integrate with existing patterns from workflow actions

### Timeout Response Format
Follow specification format:
```json
{
  "content": [{"type": "text", "text": "Command timed out after 300 seconds"}],
  "is_error": true,
  "metadata": {
    "command": "long_running_command",
    "timeout_seconds": 300,
    "partial_stdout": "...",
    "partial_stderr": "...",
    "working_directory": "/path/to/dir"
  }
}
```

## Architecture Integration

### Existing Patterns
- Study `swissarmyhammer/src/workflow/actions.rs` shell action implementation
- Reuse timeout patterns from existing code
- Follow process management patterns from test utilities
- Integrate with existing error handling hierarchy

### Cross-Platform Considerations
- Handle Unix signal-based termination (SIGTERM, SIGKILL)
- Handle Windows process termination properly
- Test process cleanup on both platforms
- Account for platform-specific process behavior

## Acceptance Criteria

- [ ] Timeout functionality works correctly with configurable durations
- [ ] Process cleanup prevents orphaned processes
- [ ] Timeout errors return partial output when available
- [ ] ProcessGuard integration prevents resource leaks
- [ ] Cross-platform process termination works
- [ ] Process tree termination handles nested processes
- [ ] Timeout response format matches specification
- [ ] Integration with existing workflow patterns maintained

## Testing Requirements

- [ ] Unit tests for various timeout scenarios
- [ ] Tests for process cleanup and orphan prevention
- [ ] Cross-platform compatibility testing
- [ ] Integration tests with long-running commands
- [ ] Memory leak testing for process management
- [ ] Signal handling and cancellation testing

## Notes

- This step builds on the core execution engine from the previous step
- Focus on reliability and preventing resource leaks
- Ensure compatibility with existing workflow system patterns
- Partial output capture during timeout is important for debugging
- Process tree termination is crucial for system stability