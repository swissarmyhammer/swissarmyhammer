---
position_column: done
position_ordinal: i1
title: Make resolve_local visibility pub(crate)
---
**model-loader/src/loader.rs**

`resolve_local` is `pub` but only used internally. Reduce visibility to `pub(crate)` or make it private.

- [ ] Change `pub fn resolve_local` to `fn resolve_local` or `pub(crate) fn resolve_local`
- [ ] Verify tests pass #review-finding