---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: todo
position_ordinal: d280
project: spatial-nav
title: 'Spatial-nav #4: nav.first / nav.last = focus first/last child'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting.

**This component owns:** `nav.first` (Home) and `nav.last` (End). Focus the focused scope's first / last child. On a leaf, no-op.

**Contract (restated from design):**

> First child = the child whose rect is topmost; ties broken by leftmost.
> Last child = the child whose rect is bottommost; ties broken by rightmost.
> Children = registered scopes whose `parent_zone` is the focused scope's FQM.

`nav.first` is identical to `nav.drillIn` (component #2) when the focused scope has children. They share an implementation; the only difference is the key (Enter vs Home) and the React-side editor-focus extension on Enter (which `nav.first` does not get).

## What

### Files to modify

- `swissarmyhammer-focus/src/navigate.rs`:
  - Refactor `edge_command` (line 640). Currently `Direction::First` / `Last` operate on the focused entry's *siblings* (so `Home` in a row of cells = "first cell in that row"). New contract: `First` / `Last` operate on the focused scope's *children* (so `Home` on a focused row container = "first cell in this row"; on a focused leaf = no-op).
  - Concretely: `Direction::First` returns first-child by topmost-then-leftmost ordering; `Direction::Last` returns last-child by bottommost-then-rightmost ordering. Drop the `expect_zone` filter — kind doesn't matter.
  - Decide fate of `Direction::RowStart` / `Direction::RowEnd`. Recommend dropping them (or keeping as aliases for `First` / `Last`) — the user's model has no separate "first in row" concept; the focused scope IS the row. If dropped, sweep callers.

- `swissarmyhammer-focus/src/types.rs`:
  - Update `Direction` enum docstrings for `First` / `Last`. Decide on `RowStart` / `RowEnd` removal.

- `swissarmyhammer-focus/README.md`:
  - Add / update a "## First / Last" section describing the contract. Note the difference from old behaviour if `RowStart` / `RowEnd` were dropped.

### Tests

- **Unit test in `swissarmyhammer-focus/src/navigate.rs::tests` or new `tests/first_last_child.rs`**:
  - Focused leaf → `nav.first` and `nav.last` return focused FQM (no-op).
  - Focused scope with one child → both return the child.
  - Focused scope with three children in a row → `first` returns leftmost, `last` returns rightmost.
  - Focused scope with three children in a column → `first` returns topmost, `last` returns bottommost.
  - Focused scope with mixed leaves and sub-zones → both consider all children regardless of kind.
- **Sweep existing tests** that use `Direction::First` / `Last` / `RowStart` / `RowEnd` — update or remove per the new contract. The `unified_trajectories.rs` and `column_header_arrow_nav.rs` tests are the most likely affected.
- Run `cargo test -p swissarmyhammer-focus` and confirm green.

## Acceptance Criteria

- [ ] `nav.first` and `nav.last` operate on the focused scope's children, not siblings.
- [ ] On a leaf (no children), both return focused FQM (no-op).
- [ ] On a container with children, `first` = topmost-then-leftmost, `last` = bottommost-then-rightmost.
- [ ] `Direction::RowStart` / `RowEnd` decision documented (kept as aliases or removed; sweep complete either way).
- [ ] README "## First / Last" section captures the contract.
- [ ] Existing tests pass unchanged or are updated with rationale.
- [ ] `cargo test -p swissarmyhammer-focus` passes.

## Workflow

- Use `/tdd`. Write the new first/last child unit tests, refactor `edge_command`, sweep the `Direction` enum and existing tests.
#spatial-nav-redesign