---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: ef80
project: spatial-nav
title: 'motion-validation: nav.drillOut — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.drillOut`** (the Escape binding) that pins drill-out semantics across leaf, top-level, layer-root, and modal-fall-through paths. Family 3 in `spatial-nav-end-to-end.spatial.test.tsx` has one Escape case (asserts the dispatched IPC); this file goes deeper.

Bindings exercised: `Escape` (vim, cua, emacs — all three modes) — drill is built in `buildDrillCommands` at `kanban-app/ui/src/components/app-shell.tsx:344-389` (drillOut closure at lines 365-388). The closure awaits `actions.drillOut(focusedFq, focusedFq)` → `spatial_drill_out` Tauri command. When the kernel echoes the focused FQM (layer-root, torn state) the closure falls through to `app.dismiss` to close the topmost modal layer.

Kernel behavior under test: `SpatialRegistry::drill_out` in `swissarmyhammer-focus/src/registry.rs`. Returns `registry.scopes[focused].parent_zone` when present; echoes focused FQM at the layer root.

### File to create

`kanban-app/ui/src/spatial-nav-drillout.spatial.test.tsx` — same harness pattern as the cardinal validation tasks.

### Scenarios (one `it()` each)

- [ ] **leaf → parent_zone** — focus `task:T1` (a leaf inside column TODO); press Escape; assert (a) `spatial_drill_out` is dispatched, (b) IPC result is the column TODO's FQ, and (c) post-drill `data-focused` is on `board:column:TODO`.
- [ ] **last_focused recorded on drill-out** — focus T1, navigate to T2 (Down), drill out (Escape) to column TODO, drill back in (Enter). Assert the warm-start lands on T2 (the last_focused-by-FQ recorded entry). This is the symmetric proof that drill-out doesn't clobber `last_focused_by_fq`.
- [ ] **top-level scope falls through to app.dismiss** — focus a top-level scope whose `parent_zone` is None (layer root); press Escape; assert (a) `spatial_drill_out` IPC was dispatched and result equals the focused FQM (echoed), and (b) `app.dismiss` was dispatched as the fall-through. Use `mockInvoke.mock.calls` to assert order.
- [ ] **modal layer pop on Escape** — open the inspector (modal layer); focus a field; press Escape; assert that focus exits the inspector layer cleanly. Either the field's `parent_zone` walks up within the inspector OR (when at the inspector's layer root) `app.dismiss` closes the inspector. Pin which path is correct against the inspector's actual scope tree.
- [ ] **command palette Escape closes the palette** — open the palette (`Mod+Shift+P`); press Escape; assert the palette closes, and `nav.drillOut` does NOT propagate to spatial focus (the palette has its own captured-focus layer that swallows Escape).
- [ ] **rename editor Escape cancels rename** — start a perspective-tab rename (Family 5 path); press Escape inside the rename input; assert rename cancels and focus returns to the perspective tab — `nav.drillOut` does NOT fire because the editor's scope-level `editor.cancel` shadows Escape.
- [ ] **vim Escape parity** — vim mode; press Escape from a leaf; identical dispatch to cua mode.

### Out of scope

- Do NOT modify kernel code.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-drillout.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.drillOut — production app", () => { ... })`.
- [ ] Each scenario asserts (a) dispatched `spatial_drill_out` IPC shape (or absence — for shadowed Escape paths), (b) result FQM where dispatched, and (c) post-keydown `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-drillout`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-drillout.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-drillout`.
- [ ] Existing spatial tests still pass.

## Workflow

- Use `/tdd` — write each scenario as a failing assertion first.

#motion-validation #stateless-rebuild