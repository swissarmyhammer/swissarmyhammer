---
position_column: done
position_ordinal: i0
title: Remove unused _start_time in resolve()
---
**model-loader/src/loader.rs**

`_start_time` variable in `resolve()` is assigned but never used. Remove it or use it for timing metadata.

- [ ] Remove `_start_time` or wire it into `ModelMetadata::resolve_time`
- [ ] Verify tests pass #review-finding