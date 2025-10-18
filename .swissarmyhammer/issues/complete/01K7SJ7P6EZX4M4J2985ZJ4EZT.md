# Add Progress Notifications to shell_execute Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**HIGH** - Shell commands are frequently long-running operations

## Summary
Add real-time streaming progress notifications to the shell_execute tool, sending each line of stdout/stderr as it arrives instead of waiting for command completion.

## Location
`swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs`

## Current Behavior
- Executes shell command and buffers all output
- Returns complete stdout/stderr only after command finishes
- No feedback during long-running commands (builds, tests, deployments)
- Users see nothing until command completes

## Implementation Analysis

After examining the code, here's my analysis of where to add notifications:

### Key Functions
1. **`McpTool::execute` (line 1217)**: The MCP tool entry point with access to `ToolContext`
2. **`execute_shell_command` (line 1040)**: Core execution logic (doesn't have access to context)
3. **`process_child_output_with_limits` (line 806)**: The streaming output processing loop

### Architecture Decision
Since `process_child_output_with_limits` doesn't have access to the `ToolContext`, I need to:
1. Pass the `ProgressSender` down through the call chain
2. Generate the progress token at the start of execution  
3. Send notifications from within `process_child_output_with_limits` where the streaming happens

## Proposed Solution

### Step 1: Update Function Signatures

Add optional `ProgressSender` parameter to:
```rust
async fn execute_shell_command(
    command: String,
    working_directory: Option<PathBuf>,
    environment: Option<std::collections::HashMap<String, String>>,
    progress_sender: Option<&ProgressSender>,  // NEW
    progress_token: &str,  // NEW
) -> Result<ShellExecutionResult, ShellError>
```

```rust
async fn process_child_output_with_limits(
    mut child: Child,
    output_limits: &OutputLimits,
    progress_sender: Option<&ProgressSender>,  // NEW
    progress_token: &str,  // NEW
) -> Result<(std::process::ExitStatus, OutputBuffer), ShellError>
```

### Step 2: Send Start Notification

In `McpTool::execute`, before calling `execute_shell_command`:
```rust
// Generate progress token for this execution
let progress_token = generate_progress_token();

// Send start notification
if let Some(sender) = &_context.progress_sender {
    sender.send_progress(
        &progress_token,
        Some(0),
        format!("Executing: {}", request.command)
    ).ok();
}
```

### Step 3: Stream Output Notifications

In `process_child_output_with_limits`, in the tokio::select! loop where lines are read:
```rust
// Read from stdout
stdout_line = stdout_reader.next_line() => {
    match stdout_line {
        Ok(Some(line)) => {
            // Send progress notification for this line
            if let Some(sender) = progress_sender {
                sender.send_progress(
                    progress_token,
                    None,  // Indeterminate progress
                    &line
                ).ok();
            }
            
            // Existing buffering logic...
        }
    }
}
```

### Step 4: Send Completion Notification

At the end of `execute_shell_command`, after creating the result:
```rust
// Send completion notification
if let Some(sender) = progress_sender {
    sender.send_progress_with_metadata(
        progress_token,
        Some(100),
        format!("Command completed with exit code {}", result.exit_code),
        json!({
            "exit_code": result.exit_code,
            "duration_ms": result.execution_time_ms,
            "output_truncated": result.output_truncated
        })
    ).ok();
}
```

### Step 5: Import Required Dependencies

At the top of the file:
```rust
use crate::mcp::progress_notifications::{ProgressSender, generate_progress_token};
use serde_json::json;
```

## Testing Strategy

### Test 1: Progress notifications are sent
```rust
#[tokio::test]
async fn test_shell_execute_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    
    let mut context = create_test_context();
    context.progress_sender = Some(progress_sender);
    
    let tool = ShellExecuteTool::new();
    let mut args = serde_json::Map::new();
    args.insert("command".to_string(), serde_json::Value::String("echo 'line1'; echo 'line2'".to_string()));
    
    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());
    
    // Collect notifications
    let mut notifications = Vec::new();
    while let Ok(notif) = rx.try_recv() {
        notifications.push(notif);
    }
    
    // Should have start, output lines, and completion
    assert!(notifications.len() >= 3);
    assert_eq!(notifications[0].progress, Some(0)); // Start
    assert_eq!(notifications.last().unwrap().progress, Some(100)); // Complete
}
```

### Test 2: Notifications don't break execution on error
```rust
#[tokio::test]
async fn test_shell_execute_continues_when_notification_fails() {
    let (tx, rx) = mpsc::unbounded_channel();
    drop(rx); // Close channel to cause send errors
    
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let mut context = create_test_context();
    context.progress_sender = Some(progress_sender);
    
    let tool = ShellExecuteTool::new();
    let mut args = serde_json::Map::new();
    args.insert("command".to_string(), serde_json::Value::String("echo 'test'".to_string()));
    
    // Should still succeed even though notifications fail
    let result = tool.execute(args, &context).await;
    assert!(result.is_ok());
}
```

## Benefits

1. **Real-time Feedback**: Users see output as it happens
2. **Better UX**: Progress visible for long builds, tests, deployments
3. **Debugging**: Can see where command hangs or fails
4. **Standard Compliance**: Uses MCP progress notifications properly
5. **No Breaking Changes**: Optional parameter, existing code unaffected

## Success Criteria

- [ ] Start notification sent when command begins
- [ ] Output lines streamed as notifications
- [ ] Completion notification includes exit code and duration
- [ ] Command execution succeeds even if notification fails
- [ ] Tests verify notification delivery
- [ ] No performance degradation
- [ ] All existing tests still pass

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)



---

## Implementation Notes - Code Review Cleanup (2025-10-18)

### Changes Made

Fixed three temporal references in comments that violated coding standards:

1. **Line 132**: Changed "After removing the `sah_config` module, shell configuration moved to hardcoded" to "Shell configuration uses hardcoded defaults. The `sah_config` module is not used."

2. **Line 1090**: Changed "Note: Process group configuration removed for compatibility" to "Note: Process group configuration is not used for compatibility"

3. **Line 1262**: Changed "Using default shell configuration (removed sah_config dependency)" to "Using default shell configuration (does not use sah_config)"

### Test Results

All 3302 tests pass successfully:
- No clippy warnings
- No compilation errors  
- 73 shell execution tests passing
- Progress notification tests passing

### Compliance

Code now fully complies with coding standards:
- ✅ No temporal references in comments
- ✅ Comments describe current state, not past changes
- ✅ Evergreen documentation that won't become stale

The progress notification implementation remains intact and fully functional.