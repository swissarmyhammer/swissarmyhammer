---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb780
project: store-service
title: Fix misleading depth comment in txn_grouping_e2e test
---
## What

`crates/swissarmyhammer-store/tests/integration/txn_grouping_e2e.rs:85-89` has a comment that says "Depth should be 1 — the group counts as a single undo step." but the assertion is `assert_eq!(depth["depth"], json!(2))`. They directly contradict — the implementation returns raw entry count (via `UndoStack::pointer()`), so `depth==2` is correct and the comment is wrong.

## Acceptance Criteria

- [ ] Replace the misleading "Depth should be 1" comment with one that matches the assertion, e.g. "Depth reports raw entries (2), not groups; group accounting is exposed via the items array on a single undo."
- [ ] No behavior change

## Notes

Discovered during review of `01KS5F7BR6850RKT67X4CNHPAZ`. Two-line fix.