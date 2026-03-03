---
title: Remove dead CommentId type from ids.rs
position:
  column: todo
  ordinal: d4
---
`CommentId` is still defined in `types/ids.rs` but the `comment` module was deleted. This type is dead code.

- [ ] Remove `CommentId` from `types/ids.rs`
- [ ] Remove any re-exports of `CommentId`
- [ ] Verify no remaining references