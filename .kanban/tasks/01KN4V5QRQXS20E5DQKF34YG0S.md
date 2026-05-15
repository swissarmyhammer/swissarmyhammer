---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff380
title: StoreContext silently swallows undo_stack.yaml save errors
---
**swissarmyhammer-store/src/context.rs:51, 83, 117**\n\n`let _ = stack.save(...)` silently discards I/O errors when persisting the undo stack. If the save fails (disk full, permissions), the in-memory stack and on-disk stack diverge. A subsequent crash would lose undo history.\n\nAlso in `StoreContext::new()` (line 34): `UndoStack::load(...).unwrap_or_default()` silently replaces a corrupt file with an empty stack instead of surfacing the error.\n\n**Severity: warning**\n\n**Suggestion:** At minimum, log the error with `tracing::warn!`. Better: propagate the error so callers can handle it. The `push`, `undo`, and `redo` methods should return `Result<()>` already (undo/redo do), so propagating from `push` just requires changing its signature.\n\n**Subtasks:**\n- [ ] Replace `let _ = stack.save(...)` with error logging or propagation\n- [ ] Handle or log the `unwrap_or_default()` in `new()`\n- [ ] Verify the stack is persisted correctly after undo/redo" #review-finding