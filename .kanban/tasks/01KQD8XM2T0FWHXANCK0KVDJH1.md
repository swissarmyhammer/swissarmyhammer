---
assignees:
- claude-code
depends_on:
- 01KQD8X3PYXQAJN593HR11T7R4
position_column: review
position_ordinal: '8480'
project: spatial-nav
title: 'Path monikers Layer 2: Tauri command boundary + React adapter FQM rewire (bun run test:browser green)'
---
## Subset of `01KQD6064G1C1RAXDFPJVT1F46`

Second of three sequenced sub-tasks. Depends on Layer 1 (kernel newtypes) landing first.

## Status — DONE: TS compile clean; 0 vitest failures (179 files / 1849 tests pass / 4 pre-existing skipped)

### Done — Section A: Tauri command boundary (`kanban-app/src/commands.rs` + `main.rs`)

(Locked in earlier; cargo build, cargo test -p kanban-app, cargo clippy clean.)

### Done — Section B: TS branded types (`kanban-app/ui/src/types/spatial.ts`)

- New types defined: `SegmentMoniker`, `FullyQualifiedMoniker` (distinct brands), `WindowLabel`, `LayerName`, `Pixels`.
- Helpers: `asSegment`, `asFq`, `asLayerName`, `asWindowLabel`, `asPixels`, `composeFq`, `fqRoot`, `fqLastSegment`.
- Removed: `SpatialKey`, `LayerKey`, flat `Moniker`, `asMoniker`, `asSpatialKey`, `asLayerKey`.
- `FocusChangedPayload` updated: `prev_fq`, `next_fq`, `next_segment`.

### Done — Section C: React primitives + Section D: spatial-focus + entity-focus contexts + production callsite migration

(All production code compiles clean.)

### Done — TypeScript compile is CLEAN

- `cd kanban-app/ui && npx tsc --noEmit` returns 0 errors.

### Done in earlier passes — bulk-transform sweep across 49 test files

(See git log.)

### Done — New file `path-monikers.kernel-driven.browser.test.tsx` with 7 named tests, all passing

### Done in THIS pass — drove vitest from 21 failures to ZERO

#### entity-inspector.test.tsx — 6 failures
- The test mock for `invoke` did not emit `focus-changed` back through `listen`, so the React-side store never updated after `setFocus(fq)`. Switched to `installKernelSimulator` with hoisted `mockInvoke`/`mockListen` so `spatial_focus(fq)` emits a synthetic `focus-changed` event the spatial-focus-context bridge picks up.
- The `renderWithSectionedSchema` helper was clobbering the kernel simulator with a bare `mockImplementation`. Replaced with a fresh `installKernelSimulator` call carrying a sectioned-schema fallback.

#### entity-inspector.field-enter-drill.browser.test.tsx — 1 failure
- `bugPill!.focusedFq` → `bugPill!.fq` (the captured `spatial_register_scope` payload uses `fq`, not `focusedFq`).

#### app-shell.test.tsx — 2 failures
- `nav.drillOut` echo test: the kernel echoes the FQM, not a segment. Mock `spatial_drill_out` to return `asFq(focusedFq)` so the closure's equality check fires `app.dismiss`.
- "Space dispatches inspect" test: the FocusScope registers under `/window/task:t-bridge`, so the synthetic `focus-changed` payload must carry that FQM (not the legacy `k:t-bridge`) for the entity-focus bridge to find the scope.

#### grid-view.spatial-nav.test.tsx — 4 failures
- `c.moniker` → `c.segment` on captured `spatial_register_scope` payloads (two assertions).
- "clicking a cell dispatches exactly one spatial_focus" test was double-firing because `useGridCallbacks.handleCellClick` called `focusCell` redundantly. The per-cell `<FocusScope>` already calls `focus(fq)` on click — removed the redundant `focusCell` call from the inner-div handler.

#### grid-view.cursor-ring.test.tsx — 2 failures
- "cursor-ring tracks focused cell" test: `parseGridCellMoniker` rejected the FQM shape (it expected the bare `grid_cell:R:K` segment). Extended the parser to accept a fully-qualified moniker by extracting the trailing segment.
- `c.moniker` → `c.segment` on captured `spatial_register_scope` payload.

#### focus-on-click.regression.spatial.test.tsx — 1 failure
- Column-name leaf test was getting 2 `spatial_focus` calls. Found `<ColumnHeader>` had a leftover `onClickCapture` on the outer div that called `setFocus(columnNameFq)` redundantly with the inner `<FocusScope>`'s click handler. Removed the capture-phase handler.

#### fields/field.enter-edit.browser.test.tsx — 2 failures
- The harness's `defaultInvokeImpl` was reading `args.focusedMoniker` for `spatial_drill_in`, but under the FQM model the kernel takes `args.focusedFq`. The mock returned null (not the focused FQM echo), so the closure took the move-focus branch instead of the open-editor branch. Fixed to read `args.focusedFq`.

#### data-table.virtualized.test.tsx — 2 failures
- `rerender(...)` and `renderHeightTable` were rendering trees missing `<SpatialFocusProvider>` + `<FocusLayer>`, so `EntityRow`'s strict `useFullyQualifiedMoniker()` threw. Wrapped both with the spatial primitives.

#### board-integration.browser.test.tsx — 1 failure
- `scope_chain.some((s) => s.startsWith("task:"))` no longer holds because chain entries are full FQM paths under the path-monikers refactor. Match the trailing segment with `(^|/)task:` regex instead.

### Failure-count progression

- Start of refactor: ~774 errors / ~80 files.
- End of previous session: 0 TS errors / 278 vitest / 59 test files failing.
- Start of last session: 0 TS errors / 258 vitest / 49 test files failing.
- End of last session: 0 TS errors / 21 vitest / 9 test files failing.
- **End of THIS session: 0 TS errors / 0 vitest failures / 4 skipped (pre-existing) / 179 files / 1849 tests pass.**

## Acceptance Criteria

- [x] Tauri commands accept FQM/segment shape.
- [x] `cargo test -p kanban-app` passes.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.
- [x] TS branded types `SegmentMoniker`/`FullyQualifiedMoniker` defined; `SpatialKey`/`LayerKey`/flat `Moniker` removed from `types/spatial.ts`.
- [x] React primitives rewritten.
- [x] `entity-focus-context` rewritten.
- [x] Test infrastructure (spatial-shadow-registry + kernel-simulator) rewritten to FQM identity.
- [x] All production code migrated — `npx tsc --noEmit` is **zero errors**.
- [x] `bun run test:browser` (and node tests) pass — **0 vitest failures**, 179 files, 1849 tests, 4 pre-existing skipped.
- [x] New file `path-monikers.kernel-driven.browser.test.tsx` with 7 named tests authored and passing.
- [x] `cargo test --workspace` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Out of scope (handled in Layer 3 card)

- `npm run tauri dev` manual log verification.

## Depends on

- Layer 1 sub-task (Rust kernel newtypes).

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
