---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffaf80
title: PerspectiveContext::delete silently ignores file removal failure
---
context.rs:117\n\n```rust\nlet _ = fs::remove_file(&path).await;\n```\n\nThe `delete` method discards the result of `fs::remove_file`. If the file cannot be deleted (permissions, I/O error), the in-memory state is updated but the YAML file remains on disk. On next `open()`, the perspective reappears -- a ghost resurrection.\n\nSuggestion: Propagate the error, or at minimum log a warning. The method already returns `Result`, so propagating is straightforward:\n```rust\nfs::remove_file(&path).await.map_err(|e| {\n    tracing::warn!(%e, id, \"failed to remove perspective file\");\n    KanbanError::Io(e)\n})?;\n```\n\nAlternatively, if the intent is to be tolerant of missing files (already deleted externally), use `if path.exists()` or match on `ErrorKind::NotFound`.\n\nVerification: Review the updated code for proper error handling. Optionally add a test that verifies delete fails gracefully on a read-only directory." #review-finding