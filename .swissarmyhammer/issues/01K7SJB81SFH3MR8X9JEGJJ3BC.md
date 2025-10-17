# Add Progress Notifications to rules_check Tool

## Parent Issue
Phase 1: Implement MCP Progress Notification Infrastructure (01K7SHZ4203SMD2C6HTW1QV3ZP)

## Priority
**LOW** - Usually fast unless checking many files

## Summary
Add progress notifications to the rules_check tool to show rule checking progress when scanning large codebases.

## Location
`swissarmyhammer-tools/src/mcp/tools/rules/check/mod.rs`

## Current Behavior
- Silently checks all files against rules
- Returns violations only after all files checked
- No feedback during checking
- Can be slow with many files or complex rules

## Proposed Notifications

### 1. Start Notification
```rust
// After parameter validation (around line 80)
if let Some(sender) = &context.progress_sender {
    let token = generate_progress_token();
    sender.send_progress_with_metadata(
        &token,
        Some(0),
        "Starting rules check",
        json!({
            "rule_names": request.rule_names,
            "file_patterns": request.file_paths,
            "category": request.category,
            "severity": request.severity
        })
    ).ok();
}
```

### 2. Rules Loaded Notification
```rust
// After loading rules (around line 100)
if let Some(sender) = &context.progress_sender {
    sender.send_progress_with_metadata(
        &token,
        Some(10),
        format!("Loaded {} rules", rules_count),
        json!({
            "rules_count": rules_count
        })
    ).ok();
}
```

### 3. Files Discovery Notification
```rust
// After discovering files to check (around line 120)
if let Some(sender) = &context.progress_sender {
    sender.send_progress_with_metadata(
        &token,
        Some(20),
        format!("Found {} files to check", total_files),
        json!({
            "total_files": total_files
        })
    ).ok();
}
```

### 4. Checking Progress Updates
```rust
// In file checking loop (around line 140)
for (i, file_path) in files.iter().enumerate() {
    // Check file against rules...
    
    if i % 10 == 0 || i == files.len() - 1 {
        if let Some(sender) = &context.progress_sender {
            let progress = 20 + ((i as f64 / total_files as f64) * 75.0) as u32;
            sender.send_progress_with_metadata(
                &token,
                Some(progress),
                format!("Checked {}/{} files ({} violations)",
                    i + 1, total_files, total_violations),
                json!({
                    "files_checked": i + 1,
                    "total_files": total_files,
                    "violations_found": total_violations,
                    "current_file": file_path.display().to_string()
                })
            ).ok();
        }
    }
}
```

### 5. Completion Notification
```rust
// After all checks complete (around line 170)
if let Some(sender) = &context.progress_sender {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    sender.send_progress_with_metadata(
        &token,
        Some(100),
        format!("Rules check complete: {} violations in {} files",
            total_violations,
            files_with_violations
        ),
        json!({
            "files_checked": files_checked,
            "violations_found": total_violations,
            "files_with_violations": files_with_violations,
            "violations_by_severity": violations_by_severity,
            "duration_ms": duration_ms
        })
    ).ok();
}
```

## Implementation Details

### Progress Breakdown
```rust
// 0%: Start
// 10%: Rules loaded
// 20%: Files discovered
// 20-95%: File checking (scales with file count)
// 95-100%: Final processing
// 100%: Complete
```

### Notification Frequency
```rust
// Send notifications:
// - Every 10 files
// - Or every 10% progress change
// - Always send for last file

const NOTIFICATION_INTERVAL_FILES: usize = 10;
```

### Violation Severity Tracking
```rust
// Track violations by severity for metadata
let mut violations_by_severity = HashMap::new();

for violation in violations {
    *violations_by_severity
        .entry(violation.severity)
        .or_insert(0) += 1;
}
```

## Code Locations

### Main Changes
1. **Line ~80**: Add start notification
2. **Line ~100**: Add rules loaded notification
3. **Line ~120**: Add files discovery notification
4. **Line ~140**: Add checking progress in loop
5. **Line ~170**: Add completion notification with statistics
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
async fn test_rules_check_sends_progress_notifications() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = Arc::new(ProgressSender::new(tx));
    let context = test_context_with_progress(progress_sender);
    
    // Create test files with violations
    let temp_dir = create_test_files_with_violations(25);
    
    // Run rules check
    let result = check_rules(
        &format!("{}/**/*.rs", temp_dir.path().display()),
        &context
    ).await;
    
    // Verify notifications
    let notifications: Vec<_> = collect_notifications(&mut rx).await;
    
    assert!(notifications.len() >= 5); // start, rules, discovery, progress, complete
    assert_eq!(notifications.first().unwrap().progress, Some(0));
    assert_eq!(notifications.last().unwrap().progress, Some(100));
}
```

## Benefits

1. **Visibility**: Users know checking is progressing
2. **Violation Tracking**: Real-time count of violations
3. **File Tracking**: Can see which file is being checked
4. **Better UX**: Feedback for large codebase checking

## Performance Considerations

- Rule checking is CPU-bound, notifications add minimal overhead
- Notification overhead: <1% of checking time
- Buffering (every 10 files) prevents notification flood

## Documentation

Update `doc/src/reference/tools.md`:
```markdown
### rules_check

Check source files against SwissArmyHammer rules with progress feedback.

**Progress Notifications**:
- Start: Rules check begins
- Rules Loaded: Number of rules loaded
- Discovery: Total files found
- Progress: Updates every 10 files or 10%
- Completion: Final violation statistics

**Example notification stream**:
```json
{"progressToken": "rules_01K7...", "progress": 0, "message": "Starting rules check"}
{"progressToken": "rules_01K7...", "progress": 10, "message": "Loaded 15 rules"}
{"progressToken": "rules_01K7...", "progress": 20, "message": "Found 67 files to check"}
{"progressToken": "rules_01K7...", "progress": 60, "message": "Checked 40/67 files (12 violations)"}
{"progressToken": "rules_01K7...", "progress": 100, "message": "Rules check complete: 18 violations in 8 files"}
```

## Success Criteria

- [ ] Start notification sent
- [ ] Rules loaded notification includes count
- [ ] Files discovery notification includes count
- [ ] Checking progress every 10 files or 10%
- [ ] Completion includes violation statistics by severity
- [ ] Tests verify notification delivery
- [ ] Checking succeeds even if notifications fail
- [ ] Performance overhead < 1%
- [ ] Documentation updated

## Related Issues
- **01K7SHZ4203SMD2C6HTW1QV3ZP**: Phase 1: Implement MCP Progress Notification Infrastructure (prerequisite)
