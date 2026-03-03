---
title: Remove dead Comment import from context.rs
position:
  column: todo
  ordinal: d5
---
The `use` block in `context.rs` still imports `Comment` from `types`, which is only needed by deprecated methods. Remove the import when the deprecated methods are cleaned up.

- [ ] Remove `Comment` import from context.rs
- [ ] Verify compilation