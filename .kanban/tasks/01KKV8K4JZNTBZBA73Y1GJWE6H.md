---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffa980
title: 'warning: apply_compute builds query_fn on every entity read'
---
**swissarmyhammer-entity/src/context.rs:1070-1085**\n\n`apply_compute` constructs a new `Arc<EntityQueryFn>` closure on every call. This method is invoked for every entity read (both `read()` and every entity in `list()`). The closure captures clones of `root` and `fields_ctx` each time.\n\nFor single entity reads this is fine. For `list()` with N entities, it creates N identical closures. Consider lifting the query_fn construction to the `list()` method and passing it through, or constructing it once lazily.\n\nNot a correctness issue — the clones are cheap (PathBuf + Arc) — but it's unnecessary allocation in a hot path.\n\n- [ ] Consider caching or hoisting the query_fn in `list()`\n- [ ] Verify performance with large entity sets