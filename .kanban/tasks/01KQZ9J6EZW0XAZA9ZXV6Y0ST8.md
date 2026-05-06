---
assignees:
- claude-code
depends_on:
- 01KQZ7VR7JK1QD5QJDDKB529JG
position_column: todo
position_ordinal: ee80
project: spatial-nav
title: 'motion-validation: nav.drillIn — production-app spatial test'
---
## What

Add a focused, production-app-mounted spatial test file dedicated to **`nav.drillIn`** (the Enter binding) that pins drill-in semantics across cold-start, warm-start, leaf no-op, and editor-shadow paths. Family 3 in `spatial-nav-end-to-end.spatial.test.tsx` has one Enter case (asserts the dispatched IPC); this file goes deeper by also asserting the post-drill focus.

Bindings exercised: `Enter` (vim, cua, emacs — all three modes) — drill is built in `buildDrillCommands` at `kanban-app/ui/src/components/app-shell.tsx:344-389`. The closure awaits `actions.drillIn(focusedFq, focusedFq)` → `spatial_drill_in` Tauri command. Result is wired into `setFocus(result)`; the kernel always returns an FQM — a leaf with no children echoes the focused FQM (idempotent).

Kernel behavior under test: `SpatialRegistry::drill_in` in `swissarmyhammer-focus/src/registry.rs`. Algorithm: (1) prefer `last_focused_by_fq.get(focused)` (warm-start), (2) fall back to `first_child_by_top_left(children_of(focused))` (cold-start, shared with `Direction::First`'s first-child path).

### File to create

`kanban-app/ui/src/spatial-nav-drillin.spatial.test.tsx` — same harness pattern as the cardinal validation tasks.

### Scenarios (one `it()` each)

- [ ] **cold-start drill-in** — fresh app mount, focus column header `board:column:TODO`; press Enter; assert (a) `spatial_drill_in` is dispatched with the column FQ as `focused`, and (b) post-drill `data-focused` is on `task:T1` (topmost-leftmost child of the column).
- [ ] **warm-start drill-in remembers last child** — focus column TODO, press Enter (lands on T1), Down (lands on T2), Escape (drill-out to column), Enter again. Assert post-drill focus is `task:T2`, NOT `task:T1` — the kernel restored the last-focused child via `last_focused_by_fq`.
- [ ] **drill-in on a leaf is idempotent** — focus `task:T1` (a leaf — no registered children); press Enter; assert `spatial_drill_in` is dispatched, the IPC result equals the focused FQM (echoed), and `data-focused` does not change.
- [ ] **editor shadow on a focused field** — focus a field cell that has an inline-edit affordance (e.g., perspective-tab rename, card-name rename); press Enter; assert the scope-level `field.edit` (or equivalent rename) command fires, NOT `nav.drillIn`. This pins the shadowing order set up in `buildDynamicGlobalCommands` at `app-shell.tsx:408-419`.
- [ ] **first equivalence** — Enter cold-start on column TODO and Home from column TODO produce the same target (both land on `task:T1`). This pins the contract documented in `swissarmyhammer-focus/src/navigate.rs:97-106` (`first_matches_drill_in_first_child_fallback` is the kernel backstop).
- [ ] **layer-root drill-in** — focus a top-level scope at the layer root; press Enter; assert it drills into the registry's first child of that scope.
- [ ] **`nav.drillIn` palette entry** — open the command palette (`Mod+Shift+P`); search "drill"; pick the entry; assert dispatched IPC is `spatial_drill_in`. Pins the palette-execution path independently of the Enter key handler.

### Out of scope

- Do NOT modify kernel code.
- Do NOT edit `spatial-nav-end-to-end.spatial.test.tsx`.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/spatial-nav-drillin.spatial.test.tsx` exists, mounts `App`, mocks Tauri via `@/test/spatial-shadow-registry`.
- [ ] All 7 scenarios above are present as separate `it()` blocks inside one `describe("nav.drillIn — production app", () => { ... })`.
- [ ] Each scenario asserts (a) dispatched `spatial_drill_in` IPC shape, (b) result FQM, and (c) post-drill `data-focused` element.
- [ ] Passes under `cd kanban-app/ui && bun test spatial-nav-drillin`.
- [ ] No flake under 5 consecutive runs.

## Tests

- [ ] New file: `kanban-app/ui/src/spatial-nav-drillin.spatial.test.tsx`.
- [ ] Test command: `cd kanban-app/ui && bun test spatial-nav-drillin`.
- [ ] Existing kernel `first_matches_drill_in_first_child_fallback` test still passes (`cargo test -p swissarmyhammer-focus first_matches_drill_in`).
- [ ] Existing spatial tests still pass.

## Workflow

- Use `/tdd` — write each scenario as a failing assertion first.

#motion-validation #stateless-rebuild