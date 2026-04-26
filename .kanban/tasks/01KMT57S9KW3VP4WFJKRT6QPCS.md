---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffff280
title: dispatch_command_internal uses empty string fallback for cross-board drag fields
---
**Severity: Medium (Robustness)**

In `kanban-app/src/commands.rs`, the cross-board drag.complete handling extracts fields with empty-string fallbacks:

```rust
let source_path = drag_complete
    .get("source_board_path")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();
let target_path = drag_complete
    .get("target_board_path")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();
let task_id = drag_complete
    .get("task_id")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();
```

If any of these fields are missing from the DragComplete result (e.g., due to a frontend bug), the code will attempt to `resolve_handle` with an empty string path, which will either fail with a confusing error or silently do nothing.

**Recommendation:** Return an early error if any required field is missing from the DragComplete payload, with a descriptive error message indicating which field was absent. This follows the Rust review guideline that error messages must provide enough context to diagnose without reading source. #review-finding