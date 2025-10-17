# Add Progress Notifications to files_glob Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**LOW** - Usually very fast, notifications optional

## Summary
Add minimal progress notifications to the files_glob tool to show file matching progress, primarily useful for very large directory trees.

## Location
`swissarmyhammer-tools/src/mcp/tools/files/glob/mod.rs`

## Current Behavior
- Fast file pattern matching with globbing
- Returns all matching files after completion
- No feedback during matching
- Usually completes in <1 second

## Proposed Notifications

### 1. Start Notification
```rust
// After parameter validation (around line 60)
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress_with_metadata(
        &token,
        Some(0),
        format!("Matching pattern: {}", request.pattern),
        json!({
            "pattern": request.pattern,
            "path": request.path,
            "case_sensitive": request.case_sensitive,
            "respect_git_ignore": request.respect_git_ignore
        })
    ).ok();
}
```

### 2. Completion Notification
```rust
// After matching completes (around line 100)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Found {} matching files", file_count),
        json!({
            "file_count": file_count,
            "duration_ms": duration_ms
        })
    ).ok();
}
```

## Implementation Details

### Progress Breakdown
```rust
// 0%: Start
// 100%: Complete (no intermediate progress - too fast)

// Globbing is typically very fast (<1s), so we only send
// start and completion notifications
```

### Optional Intermediate Notification
```rust
// Only if matching takes longer than expected (>2 seconds)
if start_time.elapsed().as_secs() > 2 {
    if let Some(sender) = &context.progress_sender {
        sender.send_progress(
            &token,
            Some(50),
            "Matching large directory tree..."
        ).ok();
    }
}
```

## Code Locations

### Main Changes
1. **Line ~60**: Add start notification after validation
2. **Line ~100**: Add completion notification with file count
3. **Optional**: Add intermediate notification for slow operations
4. **Top of file**: Import progress utilities

### New Imports
```rust
use crate::mcp::progress_notifications::{generate_progress_token};
use serde_json::json;
```

## Testing

### Unit Tests
```rust
#[tokio::test]
async fn test_files_glob_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let context = test_context_with_progress(progress_sender);
    
    // Create test directory structure
    let temp_dir = create_test_directory_tree(50);
    
    // Run glob
    let result = glob_files(
        "**/*.rs",
        &temp_dir.path().display().to_string(),
        &context
    ).await;
    
    // Verify notifications
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    // Should have at least start and complete
    assert!(notifications.len() >= 2);
    assert_eq!(notifications.first().unwrap().progress, Some(0));
    assert_eq!(notifications.last().unwrap().progress, Some(100));
}
```

## Benefits

1. **Consistency**: All file tools send notifications
2. **Feedback**: Useful for very large directory trees
3. **Completion Stats**: Shows file count and timing

## Performance Considerations

- Glob is extremely fast, notifications are minimal
- Only 2 notifications per glob (start, complete)
- No measurable performance impact

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### files_glob

Fast file pattern matching with minimal progress feedback.

**Progress Notifications**:
- Start: Pattern matching begins
- Completion: File count and timing

**Example notification stream**:
```json
{"progressToken": "glob_01K7...", "progress": 0, "message": "Matching pattern: **/*.rs"}
{"progressToken": "glob_01K7...", "progress": 100, "message": "Found 234 matching files"}
```

**Note**: Glob operations are typically very fast (<1 second). Notifications are minimal to avoid overhead.

## Success Criteria

- [ ] Start notification sent with pattern
- [ ] Completion notification includes file count
- [ ] Tests verify notification delivery
- [ ] Glob succeeds even if notifications fail
- [ ] No performance impact
- [ ] Documentation updated

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)
