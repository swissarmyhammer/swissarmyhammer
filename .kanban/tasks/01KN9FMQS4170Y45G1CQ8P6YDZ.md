---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: 'Pattern divergence: PerspectiveContext has no StoreContext integration for undo stack'
---
**Severity**: Medium (feature gap)\n**Layer**: Design / Pattern following\n**Files**: `swissarmyhammer-perspectives/src/context.rs`, `swissarmyhammer-entity/src/context.rs:38-67`\n\n`EntityContext` holds an `OnceLock<Arc<StoreContext>>` (line 40) and uses `set_store_context()` to wire in the shared undo stack. On write/delete, it pushes undo entries:\n```rust\nif let (Some(sc), Some(eid)) = (self.store_context.get(), &entry_id) {\n    sc.push(*eid, label, item_id).await;\n}\n```\n\n`PerspectiveContext` has no `StoreContext` field at all. Even though `state.rs` registers the perspective StoreHandle with StoreContext, the push-on-write/delete step is missing. This means perspective mutations do not appear in the undo stack even though the changelog records them.\n\nThis is the most significant pattern divergence. Either perspectives should participate in undo or there should be a documented reason they don't.\n\n**Fix**: Add a `store_context: OnceLock<Arc<StoreContext>>` field and `set_store_context()` method to `PerspectiveContext`, then push entries in write/delete. Mirror EntityContext lines 218-226 and 262-269." #review-finding