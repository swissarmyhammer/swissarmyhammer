---
position_column: done
position_ordinal: i2
title: Rename get_metadata() to metadata()
---
**llama-embedding/src/model.rs**

`get_metadata()` uses the `get_` prefix which is not idiomatic Rust. Convention is to use bare `metadata()`.

- [ ] Rename `get_metadata()` to `metadata()`
- [ ] Update any callers
- [ ] Verify tests pass #review-finding