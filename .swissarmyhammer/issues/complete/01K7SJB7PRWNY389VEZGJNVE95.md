# Add Progress Notifications to outline_generate Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**MEDIUM** - Outline generation for large codebases can take time

## Summary
Add progress notifications to the outline_generate tool to show file parsing progress when generating code outlines.

## Location
`swissarmyhammer-tools/src/mcp/tools/outline/generate/mod.rs`

## Current Behavior
- Silently parses all matching files
- Uses Tree-sitter to extract symbols
- Returns complete outline only after all files processed
- No feedback during parsing of large codebases

## Proposed Notifications

### 1. Start Notification
```rust
// After pattern validation (around line 90)
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress_with_metadata(
        &token,
        Some(0),
        format!("Starting outline generation: {} patterns", request.patterns.len()),
        json!({
            "patterns": request.patterns,
            "output_format": request.output_format
        })
    ).ok();
}
```

### 2. File Discovery Notification
```rust
// After discovering files (around line 110)
if let Some(sender) = &context.progress_sender {
    sender.send_progress_with_metadata(
        &token,
        Some(10),
        format!("Found {} files to parse", total_files),
        json!({
            "total_files": total_files
        })
    ).ok();
}
```

### 3. Parsing Progress Updates
```rust
// In file parsing loop (around line 130)
for (i, file_path) in files.iter().enumerate() {
    // Parse file with tree-sitter...
    
    if i % 10 == 0 || i == files.len() - 1 {
        if let Some(sender) = &context.progress_sender {
            let progress = 10 + ((i as f64 / total_files as f64) * 85.0) as u32;
            sender.send_progress_with_metadata(
                &token,
                Some(progress),
                format!("Parsing: {}/{} files ({} symbols)",
                    i + 1, total_files, total_symbols),
                json!({
                    "files_parsed": i + 1,
                    "total_files": total_files,
                    "symbols_found": total_symbols,
                    "current_file": file_path.display().to_string()
                })
            ).ok();
        }
    }
}
```

### 4. Formatting Notification
```rust
// Before formatting output (around line 160)
if let Some(sender) = &context.progress_sender {
    sender.send_progress(
        &token,
        Some(95),
        format!("Formatting outline ({} symbols)", total_symbols)
    ).ok();
}
```

### 5. Completion Notification
```rust
// After outline generation completes (around line 180)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Generated outline: {} files, {} symbols in {:.1}s",
            files_parsed,
            total_symbols,
            duration_ms as f64 / 1000.0
        ),
        json!({
            "files_parsed": files_parsed,
            "files_failed": files_failed,
            "total_symbols": total_symbols,
            "symbol_types": symbol_type_counts,
            "duration_ms": duration_ms
        })
    ).ok();
}
```

## Implementation Details

### Progress Breakdown
```rust
// 0%: Start
// 10%: File discovery complete
// 10-95%: File parsing (scales with file count)
// 95%: Formatting output
// 100%: Complete

let progress = match phase {
    Phase::Start => 0,
    Phase::Discovery => 10,
    Phase::Parsing { files_done, total_files } => {
        10 + ((files_done as f64 / total_files as f64) * 85.0) as u32
    },
    Phase::Formatting => 95,
    Phase::Complete => 100,
};
```

### Notification Frequency
```rust
// Send notifications:
// - Every 10 files
// - Or every 10% progress
// - Always send for last file

fn should_send_notification(
    files_parsed: usize,
    total_files: usize,
    last_notified: usize
) -> bool {
    files_parsed == total_files || // Last file
    files_parsed - last_notified >= 10 || // Every 10 files
    (files_parsed * 100 / total_files) != (last_notified * 100 / total_files) // 10% change
}
```

### Symbol Type Tracking
```rust
// Track symbol types for metadata
let mut symbol_counts = HashMap::new();
// Count: functions, classes, methods, structs, enums, etc.

for symbol in symbols {
    *symbol_counts.entry(symbol.kind).or_insert(0) += 1;
}
```

## Code Locations

### Main Changes
1. **Line ~90**: Add start notification after validation
2. **Line ~110**: Add file discovery notification
3. **Line ~130**: Add parsing progress in loop
4. **Line ~160**: Add formatting notification
5. **Line ~180**: Add completion notification with statistics
6. **Top of file**: Import progress utilities

### New Imports
```rust
use crate::mcp::progress_notifications::{generate_progress_token};
use serde_json::json;
use std::collections::HashMap;
```

## Testing

### Unit Tests
```rust
#[tokio::test]
async fn test_outline_generate_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let context = test_context_with_progress(progress_sender);
    
    // Create test files
    let temp_dir = create_test_rust_files(30);
    
    // Generate outline
    let result = generate_outline(
        &format!("{}/**/*.rs", temp_dir.path().display()),
        &context
    ).await;
    
    // Verify notifications
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    assert!(notifications.len() >= 5); // start, discovery, progress, format, complete
    assert_eq!(notifications.first().unwrap().progress, Some(0));
    assert_eq!(notifications.last().unwrap().progress, Some(100));
}
```

## Benefits

1. **Visibility**: Users know parsing is progressing
2. **File Tracking**: Can see which file is being parsed
3. **Symbol Counts**: Real-time count of symbols found
4. **Better UX**: Feedback for large codebase parsing

## Performance Considerations

- Tree-sitter parsing is the bottleneck, not notifications
- Notification overhead: <1% of parsing time
- Buffering strategy (every 10 files) prevents flood

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### outline_generate

Generate structured code outlines with progress feedback.

**Progress Notifications**:
- Start: Outline generation begins
- Discovery: Total files found
- Progress: Updates every 10 files or 10%
- Formatting: Preparing output format
- Completion: Final statistics

**Example notification stream**:
```json
{"progressToken": "outline_01K7...", "progress": 0, "message": "Starting outline generation: 1 patterns"}
{"progressToken": "outline_01K7...", "progress": 10, "message": "Found 45 files to parse"}
{"progressToken": "outline_01K7...", "progress": 50, "message": "Parsing: 20/45 files (234 symbols)"}
{"progressToken": "outline_01K7...", "progress": 95, "message": "Formatting outline (456 symbols)"}
{"progressToken": "outline_01K7...", "progress": 100, "message": "Generated outline: 45 files, 456 symbols in 5.2s"}
```

## Success Criteria

- [ ] Start notification sent after validation
- [ ] File discovery notification includes count
- [ ] Parsing progress every 10 files or 10%
- [ ] Formatting notification before output generation
- [ ] Completion includes full statistics
- [ ] Symbol counts tracked by type
- [ ] Tests verify notification delivery
- [ ] Parsing succeeds even if notifications fail
- [ ] Performance overhead < 1%
- [ ] Documentation updated

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)



## Proposed Solution

After analyzing the existing codebase and similar implementations (particularly `search_index` tool), I will implement progress notifications following this approach:

### Implementation Strategy

1. **Import Required Dependencies** - Add `generate_progress_token` from progress_notifications module
2. **Generate Unique Token** - Create token at start of execution for tracking all notifications
3. **Send Notifications at Key Phases** - Follow the progress breakdown outlined in the issue
4. **Handle Notification Failures Gracefully** - Use `.ok()` to ignore send failures (non-blocking)
5. **Include Rich Metadata** - Provide detailed context at each phase for debugging and monitoring

### Progress Notification Points

The implementation will send notifications at these execution points in `execute()` method:

1. **Line ~90** (after pattern validation): 0% - Starting with patterns metadata
2. **Line ~150** (after file discovery): 10% - Total files found
3. **Line ~180-200** (during parsing loop): 10-95% - Every 10 files or 10% progress change with symbol counts
4. **Line ~210** (before formatting): 95% - Formatting with total symbols
5. **Line ~230** (after completion): 100% - Full statistics including duration, files, symbols, symbol types

### Notification Frequency Logic

```rust
// Send notifications every 10 files OR when significant progress change occurs
if i % 10 == 0 || i == supported_files.len() - 1 {
    let progress = 10 + ((i as f64 / total_files as f64) * 85.0) as u32;
    sender.send_progress_with_metadata(...).ok();
}
```

### Testing Strategy

Will add comprehensive unit test `test_outline_generate_sends_progress_notifications()` that:
- Creates temp directory with multiple test files (15+ to trigger progress updates)
- Captures notifications via channel
- Verifies start (0%), completion (100%), and monotonic progress increase
- Handles cases where embedding models may not be available in test environment

### Non-Functional Requirements

- Progress notifications must not block or fail the main operation
- Overhead must be negligible (<1% of total execution time)
- All notifications use consistent token for operation tracking
- Metadata provides actionable debugging information

### Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/outline/generate/mod.rs` - Main implementation

