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

## Proposed Notifications

### 1. Start Notification
```rust
// When command execution begins (line ~280)
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress(
        &token,
        Some(0),
        format!("Executing: {}", request.command)
    ).ok();
}
```

### 2. Streaming Output (Per Line)
```rust
// In the output reading loop (around lines 300-350)
while let Some(line) = reader.next_line().await? {
    // Send notification for each line
    if let Some(sender) = &context.progress_sender {
        sender.send_progress_with_metadata(
            &token,
            None,  // Indeterminate progress
            &line,
            json!({
                "stream": "stdout",  // or "stderr"
                "line_number": line_count
            })
        ).ok();
    }
    
    // Existing buffering logic...
}
```

### 3. Completion Notification
```rust
// After command completes (line ~400)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Command completed with exit code {}", exit_code),
        json!({
            "exit_code": exit_code,
            "duration_ms": duration_ms
        })
    ).ok();
}
```

## Implementation Details

### Token Management
```rust
// Store token in a local variable for the entire execution
let progress_token = generate_progress_token();
```

### Buffering Strategy
To avoid flooding with notifications:
```rust
// Buffer lines for up to 100ms or 10 lines, whichever comes first
const BUFFER_INTERVAL_MS: u64 = 100;
const BUFFER_SIZE: usize = 10;

let mut line_buffer = Vec::new();
let mut last_send = Instant::now();

// In read loop:
line_buffer.push(line);

if line_buffer.len() >= BUFFER_SIZE || last_send.elapsed().as_millis() >= BUFFER_INTERVAL_MS {
    if let Some(sender) = &context.progress_sender {
        let combined = line_buffer.join("\n");
        sender.send_progress(&token, None, combined).ok();
    }
    line_buffer.clear();
    last_send = Instant::now();
}
```

### Error Handling
```rust
// Never fail the command execution due to notification errors
if let Some(sender) = &context.progress_sender {
    // Use .ok() to discard notification errors
    sender.send_progress(&token, Some(0), "Starting...").ok();
}
```

## Code Locations

### Main Changes
1. **Line ~280**: Add start notification when spawning command
2. **Lines 300-350**: Add streaming notifications in stdout/stderr reading loops
3. **Line ~400**: Add completion notification with exit code and duration
4. **Top of file**: Import progress notification utilities

### New Imports
```rust
use crate::mcp::progress_notifications::{generate_progress_token};
use serde_json::json;
```

## Testing

### Unit Tests
Add to existing test module:
```rust
#[tokio::test]
async fn test_shell_execute_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    
    let context = test_context_with_progress(progress_sender);
    
    // Execute command that produces output
    let result = execute_command("echo 'line1'; echo 'line2'", &context).await;
    
    // Verify notifications received
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    assert!(notifications.len() >= 3); // start + lines + completion
    assert_eq!(notifications[0].progress, Some(0)); // Start
    assert_eq!(notifications.last().unwrap().progress, Some(100)); // Complete
}
```

### Integration Tests
Test with real long-running command:
```rust
#[tokio::test]
async fn test_shell_execute_long_command_notifications() {
    // Test with sleep command or cargo build
    // Verify notifications arrive during execution, not just at end
}
```

## Benefits

1. **Real-time Feedback**: Users see output as it happens
2. **Better UX**: Progress visible for long builds, tests, deployments
3. **Debugging**: Can see where command hangs or fails
4. **Standard Compliance**: Uses MCP progress notifications properly

## Performance Considerations

- Notification sending is async and non-blocking
- Buffering prevents notification flood
- Failed notifications don't impact command execution
- Overhead: <5% based on channel communication benchmarks

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### shell_execute

Execute shell commands with real-time output streaming.

**Progress Notifications**:
- Start: Command execution begins
- Streaming: Each line of stdout/stderr as it arrives (buffered)
- Completion: Exit code and duration when command finishes

**Example notification stream**:
```json
{"progressToken": "shell_01K7...", "progress": 0, "message": "Executing: cargo test"}
{"progressToken": "shell_01K7...", "message": "   Compiling swissarmyhammer v0.1.0"}
{"progressToken": "shell_01K7...", "message": "   Running unittests src/lib.rs"}
{"progressToken": "shell_01K7...", "progress": 100, "message": "Command completed with exit code 0"}
```

## Success Criteria

- [ ] Start notification sent when command begins
- [ ] Output lines streamed as notifications
- [ ] Completion notification includes exit code and duration
- [ ] Notifications are buffered (max 10 lines or 100ms)
- [ ] Command execution succeeds even if notification fails
- [ ] Tests verify notification delivery
- [ ] No performance degradation (< 5% overhead)
- [ ] Documentation updated

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)
