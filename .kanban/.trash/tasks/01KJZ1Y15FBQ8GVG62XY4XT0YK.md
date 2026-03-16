---
position_column: done
position_ordinal: h3
title: ResolvedModel missing Clone derive
---
**model-loader/src/types.rs**

`ResolvedModel` only derives `Debug` but consumers may need to clone it (e.g., caching resolved paths). `ModelMetadata` already implements `Clone`.

**Fix:**
- [ ] Add `Clone` derive to `ResolvedModel`
- [ ] Verify tests pass #review-finding #warning