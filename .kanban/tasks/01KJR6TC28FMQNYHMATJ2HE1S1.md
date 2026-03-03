---
title: Fix atomic_write temp file collision risk in context.rs
position:
  column: done
  ordinal: d6
---
**context.rs lines 1304-1318**

`atomic_write()` uses `path.with_extension("tmp")` as the temp file name. If two concurrent processes write to the same entity, they race on the same `.tmp` file. The entity crate's `write_entity()` in `io.rs:98-102` correctly uses PID-based naming: `format!("tmp.{}", std::process::id())`.

**Suggestion:** Adopt PID-based temp naming in `context.rs` to match entity crate:

```rust
let temp_ext = format!("tmp.{}", std::process::id());
let temp_path = path.with_extension(temp_ext);
```

- [ ] Update `atomic_write()` to use PID-based temp extension
- [ ] Verify board/tag/column/swimlane/actor writes still work
- [ ] Verify tests pass #warning