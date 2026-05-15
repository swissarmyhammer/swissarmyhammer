---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9780
title: StoreContext undo/redo holds stores read lock while awaiting store operations
---
**swissarmyhammer-store/src/context.rs:67-75 and 101-109**\n\nIn `undo()` and `redo()`, the `stores` read lock is held across `await` points:\n```rust\nlet stores = self.stores.read().await;  // lock acquired\nfor store in stores.iter() {\n    if store.has_entry(&target_id).await {  // await while locked\n        store.undo_erased(&target_id).await?;  // await while locked\n```\n\nHolding an `RwLock` across `await` points can cause priority inversion -- a `register()` call (which needs a write lock) will block until all undo/redo store I/O completes. This is unlikely to deadlock (different lock instances for stack vs stores), but it degrades latency.\n\n**Severity: nit**\n\n**Suggestion:** Clone the store `Arc` out of the lock before awaiting operations, or collect matching stores first then release the lock.\n\n**Subtasks:**\n- [ ] Restructure undo/redo to release stores lock before awaiting I/O\n- [ ] Verify no regressions" #review-finding