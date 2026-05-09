---
assignees:
- claude-code
position_column: todo
position_ordinal: f080
title: Fix swissarmyhammer-focus first/last on leaf with parent_zone — sibling vs no-op contract mismatch
---
## Failing tests

- `swissarmyhammer-focus/tests/navigate.rs:988` — `first_on_leaf_returns_focused_self`
- `swissarmyhammer-focus/tests/navigate.rs:1031` — `last_on_leaf_returns_focused_self`

Both fail with the same shape:

```
thread 'first_on_leaf_returns_focused_self' panicked at swissarmyhammer-focus/tests/navigate.rs:988:5:
assertion `left == right` failed: leaf has no children — First echoes the focused FQM
  left: FullyQualifiedMoniker("/L/card/title")
 right: FullyQualifiedMoniker("/L/card/status")
```

## Root cause

Two contradictory contracts coexist in the same crate:

1. **Test contract (integration, `navigate.rs:953` / `:1003`)** — leaf with `parent_zone = Some(card)` and `Direction::First` / `Last` should echo the focused FQM (no children → no-op).
2. **Production contract (`navigate.rs::edge_command:482-485`)** — vim G/gg semantics: `First`/`Last` on a leaf with a parent zone returns the first/last sibling within `children_of(focused.parent_zone)`. That's why `status` returns `title`.

The unit-test variant (`first_last_on_leaf_returns_focused_self` at `navigate.rs:924`) registers leaves with `parent_zone = None`, which falls into the `None => reg.children_of(&focused.fq)` branch (no children → echo). It passes, hiding the bug from the unit suite. The integration variant uses the realistic case (`parent_zone = Some(card)`) and is the one that breaks.

## What to do

Decide which contract is correct (the existing kanban task `01KQQTZ7PSXEQF1WWX14ST8WRT` says children-of-focused, with no-op on leaf; that task is in `done` even though the production code wasn't aligned with it) and fix the other side. If the no-op-on-leaf contract is right, change `edge_command` to drop the `Some(parent) => children_of(parent)` fallback and just call `children_of(&focused.fq)` unconditionally. Then verify `first_matches_drill_in_first_child_fallback` and the `_with_row_of_children` / `_with_column_of_children` tests still pass.

## Pre-existing on this branch

Both failures exist at HEAD (commit `fd927c5548`) without any local edits to `swissarmyhammer-focus/`. Last touched by `f53f65bfc5` (2026-05-04) — predates the spatial-nav step 1 React change. #test-failure