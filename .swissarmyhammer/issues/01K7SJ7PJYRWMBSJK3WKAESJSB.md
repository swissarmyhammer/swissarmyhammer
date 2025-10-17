# Add Progress Notifications to search_index Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**HIGH** - Indexing large codebases can take minutes

## Summary
Add progress notifications to the search_index tool to show file indexing progress, especially important for large codebases with hundreds or thousands of files.

## Location
`swissarmyhammer-tools/src/mcp/tools/search/index/mod.rs`

## Current Behavior
- Silently indexes all files matching patterns
- No feedback during indexing
- Only returns summary after completion
- Users have no idea how long indexing will take

## Proposed Notifications

### 1. Start Notification
```rust
// Around line 97, after pattern validation
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress_with_metadata(
        &token,
        Some(0),
        format!("Starting indexing: {} patterns", request.patterns.len()),
        json!({
            "patterns": request.patterns,
            "force": request.force
        })
    ).ok();
}
```

### 2. File Count Notification
```rust
// After discovering files but before indexing
if let Some(sender) = &context.progress_sender {
    sender.send_progress_with_metadata(
        &token,
        Some(5),
        format!("Found {} files to index", total_files),
        json!({
            "total_files": total_files
        })
    ).ok();
}
```

### 3. Periodic Progress Updates
```rust
// In the indexing loop (around line 142-162)
// Send update every 10 files or every 10% of progress
let mut last_notification = 0;
const NOTIFICATION_INTERVAL: usize = 10;

for (i, file_path) in files.iter().enumerate() {
    // Index file...
    
    if i - last_notification >= NOTIFICATION_INTERVAL || 
       (i * 100 / total_files) != (last_notification * 100 / total_files) {
        
        if let Some(sender) = &context.progress_sender {
            let progress = ((i as f64 / total_files as f64) * 90.0 + 5.0) as u32;
            sender.send_progress_with_metadata(
                &token,
                Some(progress),
                format!("Indexed {}/{} files ({} chunks)", 
                    i + 1, total_files, total_chunks),
                json!({
                    "files_processed": i + 1,
                    "total_files": total_files,
                    "chunks_created": total_chunks,
                    "current_file": file_path.display().to_string()
                })
            ).ok();
        }
        last_notification = i;
    }
}
```

### 4. Completion Notification
```rust
// After indexing completes (line ~165)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Indexed {} files ({} chunks) in {:.1}s",
            report.files_successful,
            report.total_chunks,
            duration_ms as f64 / 1000.0
        ),
        json!({
            "files_indexed": report.files_successful,
            "files_failed": report.files_failed,
            "files_skipped": report.files_processed - report.files_successful - report.files_failed,
            "total_chunks": report.total_chunks,
            "duration_ms": duration_ms
        })
    ).ok();
}
```

## Implementation Details

### Progress Calculation
```rust
// Progress breakdown:
// 0%: Start
// 5%: File discovery complete
// 5-95%: File indexing (scales with file count)
// 95-100%: Finalization
// 100%: Complete

let progress = match phase {
    Phase::Start => 0,
    Phase::Discovery => 5,
    Phase::Indexing { files_done, total_files } => {
        5 + ((files_done as f64 / total_files as f64) * 90.0) as u32
    },
    Phase::Complete => 100,
};
```

### Notification Frequency
```rust
// Strategy: Balance between feedback and flood prevention
// - Minimum: Every 10 files
// - Or: Every 10% progress change
// - Or: Every 2 seconds (for very large files)

const MIN_FILES_BETWEEN_NOTIFICATIONS: usize = 10;
const MIN_PROGRESS_CHANGE: u32 = 10; // percent
const MIN_TIME_BETWEEN_MS: u64 = 2000;

fn should_send_notification(
    files_since_last: usize,
    progress_since_last: u32,
    time_since_last: Duration
) -> bool {
    files_since_last >= MIN_FILES_BETWEEN_NOTIFICATIONS ||
    progress_since_last >= MIN_PROGRESS_CHANGE ||
    time_since_last.as_millis() >= MIN_TIME_BETWEEN_MS as u128
}
```

## Code Locations

### Main Changes
1. **Line ~97**: Add start notification after validation
2. **After file discovery**: Send file count notification
3. **Line ~142-162**: Add periodic progress in indexing loop
4. **Line ~165**: Add completion notification with statistics
5. **Top of file**: Import progress notification utilities

### New Imports
```rust
use crate::mcp::progress_notifications::{generate_progress_token};
use serde_json::json;
use std::time::Duration;
```

## Testing

### Unit Tests
```rust
#[tokio::test]
async fn test_search_index_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let context = test_context_with_progress(progress_sender);
    
    // Create test files
    let temp_dir = create_test_files(50); // 50 test files
    
    // Execute indexing
    let result = index_files(&format!("{}/**/*.rs", temp_dir.path().display()), &context).await;
    
    // Collect notifications
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    // Verify notification sequence
    assert!(notifications.len() >= 5); // Start, discovery, progress updates, complete
    assert_eq!(notifications.first().unwrap().progress, Some(0));
    assert_eq!(notifications.last().unwrap().progress, Some(100));
    
    // Verify progress increases monotonically
    let progresses: Vec<_> = notifications.iter()
        .filter_map(|n| n.progress)
        .collect();
    assert!(progresses.windows(2).all(|w| w[0] <= w[1]));
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_search_index_large_codebase_notifications() {
    // Test with actual large directory (100+ files)
    // Verify notifications arrive at reasonable intervals
    // Verify final statistics match actual indexing results
}
```

## Benefits

1. **Visibility**: Users know indexing is progressing
2. **Time Estimation**: Progress % gives sense of completion time
3. **Debugging**: Can see which files are slow to index
4. **Better UX**: No more wondering if tool is frozen

## Performance Considerations

- Notification overhead: <1% of indexing time
- Buffering strategy prevents notification flood
- Progress calculation is O(1) per file
- Failed notifications don't impact indexing

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### search_index

Index files for semantic search with real-time progress feedback.

**Progress Notifications**:
- Start: Indexing begins with pattern list
- Discovery: Total files found to index
- Progress: Updates every 10 files or 10% progress
- Completion: Final statistics with timing

**Example notification stream**:
```json
{"progressToken": "index_01K7...", "progress": 0, "message": "Starting indexing: 1 patterns"}
{"progressToken": "index_01K7...", "progress": 5, "message": "Found 520 files to index"}
{"progressToken": "index_01K7...", "progress": 45, "message": "Indexed 234/520 files (1842 chunks)"}
{"progressToken": "index_01K7...", "progress": 100, "message": "Indexed 520 files (4250 chunks) in 32.1s"}
```

## Success Criteria

- [ ] Start notification sent after validation
- [ ] File discovery notification includes count
- [ ] Progress notifications every 10 files or 10%
- [ ] Completion notification includes full statistics
- [ ] Progress values increase monotonically (0→100)
- [ ] Indexing succeeds even if notifications fail
- [ ] Tests verify notification delivery and content
- [ ] Performance overhead < 1%
- [ ] Documentation updated with examples

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)
