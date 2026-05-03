---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: todo
position_ordinal: d180
project: spatial-nav
title: 'Spatial-nav #3: nav.drillOut = focus parent'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting.

**This component owns:** `nav.drillOut` (Escape key). Focus the parent scope. At the layer root, fall through to `app.dismiss`.

**Contract (restated from design):**

> Parent = the focused scope's `parent_zone`. If `parent_zone` is `None` (focused at layer root), the React glue falls through to `app.dismiss` to close the topmost modal layer.

This is likely a no-op refactor — current `drill_out` may already do this. Audit first; if it matches, the work is just confirming the contract and documenting it in the README.

## What

### Files to modify

- `swissarmyhammer-focus/src/navigate.rs` (or wherever `drill_out` lives):
  - Audit current implementation. Confirm it returns `parent_zone` or `focused_fq` (the latter when at layer root). If it does anything more (e.g. consults sibling zones, applies geometric scoring), simplify to "return parent or stay-put."
  - No-silent-dropout: at layer root, return `focused_fq`. The React glue detects this and dispatches `app.dismiss`.

- `swissarmyhammer-focus/README.md`:
  - Add / update a "## Drill out" section describing the contract.

- `kanban-app/ui/src/components/app-shell.tsx`:
  - Confirm `buildDrillCommands` Escape handler (line 366) still falls through to `app.dismiss` when `result === focusedFq`. Already does — this is a verification, not a change.

### Tests

- **Unit test in `swissarmyhammer-focus/src/navigate.rs::tests` or new `tests/drill_out_parent.rs`**:
  - Focused scope with `parent_zone = Some(p)` → `drill_out` returns `p`.
  - Focused scope at layer root (`parent_zone = None`) → `drill_out` returns focused FQM.
  - Torn state (parent_zone references unregistered FQM) → trace error and return focused FQM.
- **Existing test `swissarmyhammer-focus/tests/inspector_dismiss.rs`** — confirm Escape from inspector content still drills out then falls through to dismiss the inspector layer. Update if behaviour changed.
- Run `cargo test -p swissarmyhammer-focus inspector_dismiss drill_out` and confirm green.

## Acceptance Criteria

- [ ] `nav.drillOut` returns the focused scope's `parent_zone` when present.
- [ ] At layer root, returns focused FQM; React glue dispatches `app.dismiss`.
- [ ] `inspector_dismiss.rs` integration tests pass.
- [ ] README "## Drill out" section captures the contract.
- [ ] `cargo test -p swissarmyhammer-focus` passes.

## Workflow

- Use `/tdd`. Audit current `drill_out` first; this is likely a verify-and-document task rather than a rewrite.
#spatial-nav-redesign