---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe580
title: StoreHandle::get swallows all io::Error as NotFound
---
**swissarmyhammer-store/src/handle.rs:49-56**\n\n```rust\nlet text = tokio::fs::read_to_string(&path)\n    .await\n    .map_err(|_| StoreError::NotFound(id_str.clone()))?;\n```\n\nThe `map_err(|_| ...)` discards the original error. A permission-denied error, a UTF-8 decoding failure, or a disk I/O error will all be reported as `NotFound`. This makes debugging impossible and can cause callers to take wrong recovery paths (e.g., creating a new item when one exists but is unreadable).\n\nContrast with `read_text()` (line 383-398) which correctly distinguishes `NotFound` from other errors.\n\n**Severity: blocker**\n\n**Suggestion:** Match on `e.kind()` like `read_text` does:\n```rust\n.map_err(|e| match e.kind() {\n    std::io::ErrorKind::NotFound => StoreError::NotFound(id_str.clone()),\n    _ => StoreError::Io(e),\n})\n```\n\n**Subtasks:**\n- [ ] Fix `get()` to distinguish NotFound from other I/O errors\n- [ ] Add a test for permission-denied or other I/O error paths\n- [ ] Verify fix" #review-finding