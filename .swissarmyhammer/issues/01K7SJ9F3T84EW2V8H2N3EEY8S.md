# Add Progress Notifications to web_fetch Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**MEDIUM** - Network requests can be slow depending on URL

## Summary
Add progress notifications to the web_fetch tool to show download and conversion progress for individual URLs.

## Location
`swissarmyhammer-tools/src/mcp/tools/web_fetch/fetch/mod.rs`

## Current Behavior
- Fetches URL silently
- Converts HTML to markdown with no feedback
- Returns only after complete processing
- No indication of progress for slow sites

## Proposed Notifications

### 1. Start Notification
```rust
// At beginning of execute() method (around line 70)
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress_with_metadata(
        &token,
        Some(0),
        format!("Fetching: {}", request.url),
        json!({
            "url": request.url,
            "timeout": request.timeout
        })
    ).ok();
}
```

### 2. Connecting Notification
```rust
// Before making HTTP request (around line 90)
if let Some(sender) = &context.progress_sender {
    sender.send_progress(
        &token,
        Some(20),
        format!("Connecting to {}", url_host)
    ).ok();
}
```

### 3. Downloading Notification
```rust
// After response received, before reading body (around line 110)
if let Some(sender) = &context.progress_sender {
    let content_length = response.content_length().unwrap_or(0);
    sender.send_progress_with_metadata(
        &token,
        Some(40),
        format!("Downloading content ({} bytes)", content_length),
        json!({
            "content_length": content_length,
            "status_code": response.status().as_u16()
        })
    ).ok();
}
```

### 4. Converting Notification
```rust
// After download, before HTML conversion (around line 130)
if let Some(sender) = &context.progress_sender {
    sender.send_progress_with_metadata(
        &token,
        Some(70),
        "Converting HTML to markdown",
        json!({
            "bytes_downloaded": content.len()
        })
    ).ok();
}
```

### 5. Completion Notification
```rust
// After conversion completes (around line 150)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Fetch complete: {} chars markdown in {:.1}s",
            markdown.len(),
            duration_ms as f64 / 1000.0
        ),
        json!({
            "markdown_length": markdown.len(),
            "original_length": content.len(),
            "duration_ms": duration_ms
        })
    ).ok();
}
```

## Implementation Details

### Progress Breakdown
```rust
// 0%: Start
// 20%: Connecting to server
// 40%: Downloading content
// 70%: Converting HTML to markdown
// 100%: Complete

// For redirects, adjust progress proportionally
if is_redirect {
    // 0-20%: Following redirect
    // 20-100%: Main flow
}
```

### Redirect Handling
```rust
// If URL redirects to different host
if response.url() != original_url {
    if let Some(sender) = &context.progress_sender {
        sender.send_progress_with_metadata(
            &token,
            Some(15),
            format!("Redirected to: {}", response.url()),
            json!({
                "redirect_url": response.url().to_string()
            })
        ).ok();
    }
}
```

### Error Notification
```rust
// On fetch failure
if let Err(e) = fetch_result {
    if let Some(sender) = &context.progress_sender {
        sender.send_progress_with_metadata(
            &token,
            None,
            format!("Fetch failed: {}", e),
            json!({
                "error": e.to_string(),
                "url": request.url
            })
        ).ok();
    }
    return Err(e);
}
```

## Code Locations

### Main Changes
1. **Line ~70**: Add start notification
2. **Line ~90**: Add connecting notification
3. **Line ~110**: Add downloading notification with content length
4. **Line ~130**: Add converting notification
5. **Line ~150**: Add completion notification with statistics
6. **Top of file**: Import progress utilities

### New Imports
```rust
use crate::mcp::progress_notifications::{generate_progress_token};
use serde_json::json;
```

## Testing

### Unit Tests
```rust
#[tokio::test]
async fn test_web_fetch_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let context = test_context_with_progress(progress_sender);
    
    // Execute fetch
    let result = fetch_url("https://example.com", &context).await;
    
    // Verify notifications
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    assert!(notifications.len() >= 5); // start, connect, download, convert, complete
    assert_eq!(notifications.first().unwrap().progress, Some(0));
    assert_eq!(notifications.last().unwrap().progress, Some(100));
}

#[tokio::test]
async fn test_web_fetch_redirect_notifications() {
    // Test that redirects are reported in notifications
}

#[tokio::test]
async fn test_web_fetch_error_notification() {
    // Test that fetch errors send appropriate notifications
}
```

## Benefits

1. **Visibility**: Users know fetch is progressing
2. **Transparency**: Can see redirects and download progress
3. **Debugging**: Easier to identify slow or failing URLs
4. **Better UX**: Clear feedback for slow network requests

## Performance Considerations

- Notification overhead: negligible compared to network I/O
- No impact on fetch or conversion performance
- Failed notifications don't affect fetch results

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### web_fetch

Fetch web content and convert to markdown with progress updates.

**Progress Notifications**:
- Start: Fetch begins with URL
- Connecting: Establishing connection
- Downloading: Content download with size
- Converting: HTML to markdown conversion
- Completion: Final markdown with statistics

**Example notification stream**:
```json
{"progressToken": "fetch_01K7...", "progress": 0, "message": "Fetching: https://example.com"}
{"progressToken": "fetch_01K7...", "progress": 20, "message": "Connecting to example.com"}
{"progressToken": "fetch_01K7...", "progress": 40, "message": "Downloading content (125000 bytes)"}
{"progressToken": "fetch_01K7...", "progress": 70, "message": "Converting HTML to markdown"}
{"progressToken": "fetch_01K7...", "progress": 100, "message": "Fetch complete: 45000 chars markdown in 2.3s"}
```

## Success Criteria

- [ ] Start notification sent with URL
- [ ] Connecting notification sent
- [ ] Downloading notification includes content length
- [ ] Converting notification sent before conversion
- [ ] Redirects reported in notifications
- [ ] Completion includes statistics (sizes, duration)
- [ ] Errors reported in notifications
- [ ] Tests verify notification delivery
- [ ] Fetch succeeds even if notifications fail
- [ ] Documentation updated

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)
