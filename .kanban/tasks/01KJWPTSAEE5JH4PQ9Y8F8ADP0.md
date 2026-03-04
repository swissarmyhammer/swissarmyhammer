---
position_column: done
position_ordinal: j4
title: Return explicit error for unsupported undo/redo op types
---
**Review finding: W4 (warning)**

`swissarmyhammer-entity/src/context.rs` — `undo_single()` and `redo_single()`

The catch-all `_ => Ok(None)` for unrecognized op types (including "undo" and "redo" entries themselves) silently succeeds. If a user passes the ULID of an undo entry to undo(), it does nothing without any feedback.

- [ ] Replace `_ => Ok(None)` with explicit error for unsupported op types
- [ ] Add UnsupportedUndoOp error variant to EntityError
- [ ] Add test: undoing an undo entry returns error
- [ ] Verify fix