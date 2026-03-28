---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9880
title: cross_board::transfer_task unreachable!() when both before_id and after_id are Some
---
**Severity: Low (Robustness)**

In `swissarmyhammer-kanban/src/cross_board.rs`, the ordinal computation block has:

```rust
if before_id.is_some() || after_id.is_some() {
    // ...
    if let Some(ref_id) = before_id {
        // ...
    } else if let Some(ref_id) = after_id {
        // ...
    } else {
        unreachable!()
    }
}
```

The `unreachable!()` is logically sound because the outer `if` guarantees at least one is `Some`. However, `DropZoneDescriptor` has `beforeId` and `afterId` as mutually exclusive (the test suite verifies this), so in practice both could never be `Some` simultaneously.

The `unreachable!()` is acceptable here but will panic at runtime if the invariant is ever violated from a different caller. Per the Rust review guidelines, panics are for bugs only, and this qualifies since reaching it would indicate a logic error in the caller.

No change needed -- just noting the implicit contract. #review-finding