# Add Progress Notifications to files_grep Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**LOW** - Usually fast, but can be slow with large directory trees

## Summary
Add progress notifications to the files_grep tool to show search progress when searching through large numbers of files.

## Location
`swissarmyhammer-tools/src/mcp/tools/files/grep/mod.rs`

## Current Behavior
- Uses ripgrep to search file contents
- Returns all matches after search completes
- No feedback during search
- Can be slow with large directory trees or complex patterns

## Proposed Notifications

### 1. Start Notification
```rust
// After parameter validation (around line 70)
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress_with_metadata(
        &token,
        Some(0),
        format!("Searching for pattern: {}", request.pattern),
        json!({
            "pattern": request.pattern,
            "path": request.path,
            "case_insensitive": request.case_insensitive,
            "output_mode": request.output_mode
        })
    ).ok();
}
```

### 2. Searching Notification
```rust
// Before starting ripgrep search (around line 100)
if let Some(sender) = &context.progress_sender {
    sender.send_progress(
        &token,
        Some(25),
        "Searching files..."
    ).ok();
}
```

### 3. Results Processing Notification
```rust
// After ripgrep completes, before processing (around line 130)
if let Some(sender) = &context.progress_sender {
    sender.send_progress_with_metadata(
        &token,
        Some(75),
        format!("Processing {} matches", match_count),
        json!({
            "matches_found": match_count,
            "files_with_matches": files_count
        })
    ).ok();
}
```

### 4. Completion Notification
```rust
// After all processing completes (around line 150)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Search complete: {} matches in {} files",
            total_matches,
            files_with_matches
        ),
        json!({
            "total_matches": total_matches,
            "files_with_matches": files_with_matches,
            "duration_ms": duration_ms,
            "output_mode": request.output_mode
        })
    ).ok();
}
```

## Implementation Details

### Progress Breakdown
```rust
// 0%: Start
// 25%: Searching files (ripgrep running)
// 75%: Processing results
// 100%: Complete

// Note: Ripgrep doesn't provide incremental progress,
// so we use indeterminate progress during search phase
```

### Handling Large Result Sets
```rust
// If processing many results, send intermediate notifications
if matches.len() > 1000 {
    if let Some(sender) = &context.progress_sender {
        sender.send_progress(
            &token,
            Some(80),
            format!("Processing large result set: {} matches", matches.len())
        ).ok();
    }
}
```

## Code Locations

### Main Changes
1. **Line ~70**: Add start notification after validation
2. **Line ~100**: Add searching notification before ripgrep
3. **Line ~130**: Add results processing notification
4. **Line ~150**: Add completion notification with statistics
5. **Top of file**: Import progress utilities

### New Imports
```rust
use crate::mcp::progress_notifications::{generate_progress_token};
use serde_json::json;
```

## Testing

### Unit Tests
```rust
#[tokio::test]
async fn test_files_grep_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let context = test_context_with_progress(progress_sender);
    
    // Create test files with content
    let temp_dir = create_test_files_with_pattern(20);
    
    // Run grep
    let result = grep_files(
        "test_pattern",
        &temp_dir.path().display().to_string(),
        &context
    ).await;
    
    // Verify notifications
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    assert!(notifications.len() >= 4); // start, searching, processing, complete
    assert_eq!(notifications.first().unwrap().progress, Some(0));
    assert_eq!(notifications.last().unwrap().progress, Some(100));
}
```

## Benefits

1. **Visibility**: Users know search is running
2. **Match Tracking**: Can see how many matches found
3. **Better UX**: Feedback for large directory searches

## Performance Considerations

- Ripgrep is very fast, notifications are minimal overhead
- Only 4 notifications per search (start, searching, processing, complete)
- No impact on ripgrep performance

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### files_grep

Content-based search with ripgrep and progress feedback.

**Progress Notifications**:
- Start: Search begins with pattern
- Searching: Ripgrep searching files
- Processing: Processing search results
- Completion: Final match statistics

**Example notification stream**:
```json
{"progressToken": "grep_01K7...", "progress": 0, "message": "Searching for pattern: error"}
{"progressToken": "grep_01K7...", "progress": 25, "message": "Searching files..."}
{"progressToken": "grep_01K7...", "progress": 75, "message": "Processing 145 matches"}
{"progressToken": "grep_01K7...", "progress": 100, "message": "Search complete: 145 matches in 23 files"}
```

## Success Criteria

- [ ] Start notification sent with pattern
- [ ] Searching notification sent before ripgrep
- [ ] Processing notification includes match count
- [ ] Completion includes match and file statistics
- [ ] Tests verify notification delivery
- [ ] Search succeeds even if notifications fail
- [ ] No performance impact
- [ ] Documentation updated

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)
