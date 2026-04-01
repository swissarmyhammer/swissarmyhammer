---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff880
title: Test store erased.rs ErasedStore impl (50.0%)
---
**File**: `swissarmyhammer-store/src/erased.rs` (50.0% -- 6/12 lines)\n\n**What**: The `ErasedStore` impl for `StoreHandle<S>` has 4 methods. `root()` and `flush_changes()` are covered. `has_entry()` (L55-56), `undo_erased()` (L59-61), and `redo_erased()` (L64-66) are uncovered.\n\n**Root cause**: No tests call these methods through the `ErasedStore` trait -- all existing tests call methods directly on `StoreHandle`. These trait methods are used by `StoreContext` for heterogeneous dispatch.\n\n**Acceptance criteria**: Coverage above 80% for erased.rs\n\n**Tests to add**:\n- Cast a `StoreHandle<MockStore>` to `dyn ErasedStore` and call `has_entry()`\n- Call `undo_erased()` through the trait and verify the item was reverted\n- Call `redo_erased()` through the trait and verify the item was re-applied" #coverage-gap