# MCP Tools Reference

SwissArmyHammer provides a comprehensive set of MCP (Model Context Protocol) tools for workflow management, issue tracking, and system control. These tools enable seamless integration with Claude Code and provide powerful automation capabilities.

## Abort Tool

The `abort` tool provides controlled termination of workflows, prompts, and processes using a robust file-based approach.

### Overview

The abort tool replaces the legacy string-based "ABORT ERROR" detection system with a reliable file-based mechanism. When invoked, it creates a `.swissarmyhammer/.abort` file that is detected by the workflow executor and CLI components for immediate termination.

### Parameters

- **`reason`** (required): String describing why the abort was necessary

### Usage Examples

#### User Cancellation
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "User cancelled the destructive operation"
  }
}
```

#### Safety Violation
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Detected potentially unsafe file system operation"
  }
}
```

#### Prerequisites Not Met
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "Required database connection could not be established"
  }
}
```

#### System Inconsistency
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "System state corruption detected, cannot continue safely"
  }
}
```

### How Abort Works

1. **File Creation**: The abort tool creates `.swissarmyhammer/.abort` with the provided reason
2. **Detection**: The workflow executor checks for this file before each state transition
3. **Error Propagation**: When detected, an `ExecutorError::Abort` is raised with the abort reason
4. **Termination**: The CLI handles the error and exits with appropriate exit code
5. **Cleanup**: Abort files are automatically cleaned up when new workflows start

### File-Based Abort Benefits

- **Robust**: Works across process boundaries and different execution contexts
- **Language Agnostic**: Any process can detect and respond to abort files
- **Atomic**: File creation is an atomic operation ensuring consistent state
- **Persistent**: Abort state survives process crashes and restarts
- **Testable**: Easy to simulate abort conditions in tests

### Integration with Workflow System

The abort tool integrates seamlessly with SwissArmyHammer's workflow system:

#### Workflow Executor Integration
```rust
// Abort detection in execute_state_with_limit
if std::path::Path::new(".swissarmyhammer/.abort").exists() {
    let reason = std::fs::read_to_string(".swissarmyhammer/.abort")
        .unwrap_or_else(|_| "Unknown abort reason".to_string());
    return Err(ExecutorError::Abort(reason));
}
```

#### CLI Error Handling
```rust
match workflow_result {
    Err(ExecutorError::Abort(reason)) => {
        tracing::error!("Workflow aborted: {}", reason);
        std::process::exit(EXIT_ERROR);
    }
    // ... handle other errors
}
```

#### Automatic Cleanup
```rust
// WorkflowRun::new cleans up abort files
if let Err(e) = std::fs::remove_file(".swissarmyhammer/.abort") {
    if e.kind() != std::io::ErrorKind::NotFound {
        tracing::warn!("Failed to clean up abort file: {}", e);
    }
}
```

### When to Use the Abort Tool

#### Recommended Use Cases
- **User Cancellation**: User explicitly requests operation cancellation
- **Safety Violations**: Potentially destructive operations detected
- **Prerequisites Missing**: Required conditions cannot be met
- **System Inconsistency**: System state corruption detected
- **Policy Violations**: Operations violating user or system policies

#### Best Practices
- **Clear Reasons**: Provide descriptive abort reasons for debugging
- **Early Detection**: Check abort conditions as early as possible
- **Graceful Termination**: Use abort for controlled shutdown rather than crashes
- **User Communication**: Include user-friendly messages in abort reasons

### Error Handling

The abort tool is designed to always succeed to ensure proper error propagation:

- **File Creation Success**: Abort file is created with the provided reason
- **Directory Creation**: `.swissarmyhammer/` directory is created if needed
- **Error Recovery**: If file creation fails, the tool still reports success
- **Logging**: File creation issues are logged for debugging

### Testing Abort Functionality

#### Creating Test Abort Files
```rust
fn create_abort_file(reason: &str) -> Result<()> {
    std::fs::create_dir_all(".swissarmyhammer")?;
    std::fs::write(".swissarmyhammer/.abort", reason)?;
    Ok(())
}
```

#### Verifying Abort Detection
```rust
fn assert_abort_file_exists(expected_reason: &str) -> Result<()> {
    let path = Path::new(".swissarmyhammer/.abort");
    assert!(path.exists(), "Abort file should exist");
    let content = std::fs::read_to_string(path)?;
    assert_eq!(content, expected_reason, "Abort reason should match");
    Ok(())
}
```

#### Cleanup Between Tests
```rust
fn cleanup_abort_file() {
    if let Err(e) = std::fs::remove_file(".swissarmyhammer/.abort") {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("Warning: Failed to cleanup abort file: {}", e);
        }
    }
}
```

### Migration from Legacy System

The abort tool replaces the legacy string-based "ABORT ERROR" detection:

#### Legacy Pattern (Deprecated)
```rust
// Old string-based detection - NO LONGER USED
if error_message.contains("ABORT ERROR") {
    std::process::exit(EXIT_ERROR);
}
```

#### Modern Pattern (Current)
```rust
// New file-based detection
if std::path::Path::new(".swissarmyhammer/.abort").exists() {
    let reason = std::fs::read_to_string(".swissarmyhammer/.abort")?;
    return Err(ExecutorError::Abort(reason));
}
```

### Performance Considerations

- **File I/O**: Abort detection requires minimal file system operations
- **Check Frequency**: Abort files are checked before each workflow state transition
- **Cleanup Overhead**: Automatic cleanup adds minimal startup overhead
- **Concurrent Safety**: File operations are atomic and thread-safe

## Future Enhancements

The abort tool foundation supports future enhancements:

- **Abort Metadata**: Timestamps, process IDs, user information
- **Abort Callbacks**: Custom cleanup actions before termination
- **Abort Propagation**: Network-based abort signaling
- **Recovery Mechanisms**: Checkpoint and resume functionality

This file-based abort system provides a robust, testable, and extensible foundation for controlled workflow termination while maintaining simplicity and reliability.