---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
project: spatial-nav
title: Row label click logs ui.setFocus success but visible focus does not update
---
## What

Reported behavior: clicking a row label leaf in the grid fires the kernel command and the log line shows `cmd=ui.setFocus` completing with a `ScopeChain` that ends in `row_label:1` — i.e. the kernel believes focus moved — yet the row label leaf's visible focus indicator does not paint. The `data-focused` attribute on `[data-segment="row_label:1"]` does not flip to `"true"` after the click.

Sample log:
```
0x140368   Default     0x0   …   cmd=ui.setFocus result={"ScopeChain":["row_label:1","project:single-changelog","ui:grid","ui:view","view:01JMVIEW0000000000PGRID0","ui:perspective","perspective:01KNF7T1EF6Z8HQGT3YZ908DF7","perspective:01KNF7T1EF6Z8HQGT3YZ908DF7","board:board","store:/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/.kanban","mode:normal","window:board-01kqdzgz26ejbrdg2h9nxce6te","engine"]} undoable=false
```

### Expected

Clicking a row label leaf must:
1. Move kernel focus to the `row_label:{di}` leaf's FQM.
2. Emit a `focus-changed` event whose `next_fq` is that leaf's FQM.
3. Cause `<FocusScope>`'s `useFocusClaim(fq, …)` for that leaf to flip its local `focused` state.
4. Render `data-focused="true"` on the leaf's root `<div>` and paint the `<FocusIndicator>`.

### Observed

Step 1 is happening (the log proves the kernel knows about `row_label:1`). Step 4 is not. Step 2 or 3 is broken in between.

### Likely shape of the bug

Two paths fire on a row-label click and they currently appear to disagree about which IPC actually moves spatial focus:

- The `<FocusScope>` outer-div click handler in `kanban-app/ui/src/components/focus-scope.tsx` (handleClick around line 410) calls `focus(fq)` → `useSpatialFocusActions().focus` → `invoke("spatial_focus", { fq })` (kanban-app/ui/src/lib/spatial-focus-context.tsx:363).
- The legacy / cursor path can dispatch `ui.setFocus` via `dispatch_command` — that's the command name visible in the log and it's also the command the spatial-focus bridge dispatches forward to advance scope chains (per the comment at spatial-focus-context.tsx:81).

If the inner `<div onClick={onClick}>` in `RowSelector` (data-table.tsx:1096) is dispatching `ui.setFocus` *and* `spatial_focus` is either (a) not being called or (b) being called with an FQM that no `useFocusClaim` is subscribed to, then the kernel's scope chain would update (matching the log) but the per-FQM claim listeners would never fire and `data-focused` would stay unset.

Candidate root causes to investigate, in priority order:

1. **`spatial_focus` is never invoked on the row-label click.** Verify the inner `<div onClick={onClick}>` does not stop propagation before reaching `<FocusScope>`'s outer div, and confirm `[FocusScope] focus failed` does not appear in the console. If `spatial_focus` is missing, only the `ui.setFocus` cursor side-effect runs, which never wakes per-FQM claim listeners. Check `useGridCallbacks.handleCellClick` (grid-view.tsx:650 — currently a no-op) and the row's `<EntityRow>` `onClick` chain.
2. **`spatial_focus` is invoked but with the wrong FQM.** The `RowSelector`'s `<FocusScope moniker={moniker}>` composes its FQM from the row's `<FocusScope renderContainer={false}>` parent — confirm the click handler reads the same composed FQM that was registered with `spatial_register_scope`. A mismatch would cause the kernel to log a focus move on the segment chain but no `useFocusClaim` would match.
3. **`focus-changed` fires but the FQM-keyed listener registry is keyed differently.** Inspect the `registerClaim` / `useFocusClaim` lookup — if the leaf's claim is registered under a stale FQM (e.g. the row index changed, registration is cached) the event is delivered to nobody.
4. **`ui.setFocus` is dispatched as a side-effect of `spatial_focus` and is the one logged**, but the spatial bridge is not relaying `focus-changed` to the React `<FocusScope>` for `row_label:N` leaves specifically. Compare with grid-cell leaves (`grid_cell:{di}:{colKey}`) — clicking those does paint the indicator. Diff the registration paths in `data-table.tsx::GridCellFocusable` vs `data-table.tsx::RowSelector`.

### Files to read first

- `kanban-app/ui/src/components/data-table.tsx` — `RowSelector` (line 1053), `EntityRow`, `GridCellFocusable` for the working comparison.
- `kanban-app/ui/src/components/focus-scope.tsx` — `SpatialFocusScopeBody` `handleClick` (line 410) and `useFocusClaim` subscription (line 324).
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `focus` action (line 363), `focus-changed` listener wiring, `registerClaim` registry.
- `kanban-app/ui/src/components/grid-view.tsx` — `useGridCallbacks` (line 639) and `handleCellClick` no-op.
- The row's outer `<FocusScope moniker={asSegment(entityMk)} renderContainer={false}>` wrapper — confirm `renderContainer={false}` does not break FQM composition for the inner `RowSelector` leaf.

### Likely fix shape

Either:
- Wire the row-label click so that `spatial_focus(fq)` is the focus IPC (and `ui.setFocus`'s scope-chain forwarding is the downstream effect), with the per-FQM `useFocusClaim` listener properly registered under the same composed FQM; **or**
- Subscribe the row-label leaf's visible-focus state to the `focus-changed` event on the `next_segment` field (`row_label:{di}`) when the FQM-form lookup misses, so the indicator paints regardless of which IPC path moved focus.

Resist the temptation to special-case row labels in the bridge — fix the underlying registration / dispatch mismatch so all leaves use one path. The grid cell leaves work today; the row label leaves should use the exact same shape.

## Acceptance Criteria

- [ ] Mouse-clicking a row label leaf in the running app sets `data-focused="true"` on the corresponding `[data-segment="row_label:{di}"]` element synchronously after the kernel `focus-changed` event arrives.
- [ ] The visible `<FocusIndicator>` paints around the row label cell after the click (no other interaction needed).
- [ ] The fix does not introduce a duplicate `spatial_focus` invoke per click (no IPC double-fire). Verify by counting `spatial_focus` calls in the unit test below.
- [ ] Clicking a grid data cell still works (regression check) — its `data-focused` flips and the `<FocusIndicator>` paints.

## Tests

- [ ] **Regression test (vitest)** in `kanban-app/ui/src/components/data-table.row-label-focus.spatial.test.tsx`: add `it("clicking a row label flips data-focused on the matching row_label leaf", …)` that mounts `<GridHarness>` with two tasks, locates the `[data-segment="row_label:0"]` element via the FQM captured in `fqToSegment`, fires `fireEvent.click` on it (or on its inner click wrapper), waits for the simulated `focus-changed` event to flush, and asserts `[data-moniker="${row0Fq}"][data-focused="true"]` is present. The test must also assert `mockInvoke` saw exactly one `spatial_focus` call with `{ fq: row0Fq }`.
- [ ] **Run the new test red first** to prove it reproduces the bug, then green after the fix. The two existing tests in the same file (`registers a row_label FocusScope leaf for every data row`, `driving focus to a row label leaf flips its data-focused attribute`) cover registration and the simulator-driven path; the new test pins the click → focus → indicator path end-to-end.
- [ ] Run `pnpm -C kanban-app/ui test data-table.row-label-focus` and confirm all three tests pass.
- [ ] Run `pnpm -C kanban-app/ui test grid-view.cursor-ring` to confirm the cell-click cursor-ring path still passes (regression check that the fix didn't disturb the working grid-cell focus path).

## Workflow

- Use `/tdd` — write the failing click-flips-data-focused regression test first against the current code (it should reproduce the bug), then identify the dispatch-vs-registration mismatch, fix it, and confirm the test goes green plus the existing suite stays clean.
