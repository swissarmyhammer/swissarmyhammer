---
assignees:
- claude-code
depends_on:
- 01KQD8X3PYXQAJN593HR11T7R4
position_column: doing
position_ordinal: '80'
project: spatial-nav
title: 'Path monikers Layer 2: Tauri command boundary + React adapter FQM rewire (bun run test:browser green)'
---
## Subset of `01KQD6064G1C1RAXDFPJVT1F46`

Second of three sequenced sub-tasks. Depends on Layer 1 (kernel newtypes) landing first.

## Status — Sections A-D + production-callsite migration done; Section E (test sweep + new test file) NOT done

### Done — Section A: Tauri command boundary (`kanban-app/src/commands.rs` + `main.rs`)

(Locked in earlier; cargo build, cargo test -p kanban-app, cargo clippy clean.)

### Done — Section B: TS branded types (`kanban-app/ui/src/types/spatial.ts`)

- New types defined: `SegmentMoniker`, `FullyQualifiedMoniker` (distinct brands), `WindowLabel`, `LayerName`, `Pixels`.
- Helpers: `asSegment`, `asFq`, `asLayerName`, `asWindowLabel`, `asPixels`, `composeFq`, `fqRoot`, `fqLastSegment`.
- Removed: `SpatialKey`, `LayerKey`, flat `Moniker`, `asMoniker`, `asSpatialKey`, `asLayerKey`.
- `FocusChangedPayload` updated: `prev_fq`, `next_fq`, `next_segment`.

### Done — Section C: React primitives

- `fully-qualified-moniker-context.tsx` — `FullyQualifiedMonikerContext`, `useFullyQualifiedMoniker`, `useOptionalFullyQualifiedMoniker`, **NEW: `useChildFq(segment)`** convenience helper.
- `layer-fq-context.tsx` — `LayerFqContext`, `useEnclosingLayerFq`, `useOptionalEnclosingLayerFq`.
- `focus-layer.tsx`, `focus-zone.tsx`, `focus-scope.tsx` rewritten — take `SegmentMoniker`, compose own FQM via context.
- `use-track-rect-on-ancestor-scroll.ts` updated.

### Done — Section D: spatial-focus + entity-focus contexts

- `lib/spatial-focus-context.tsx` rewritten — actions surface takes FQM.
- `lib/entity-focus-context.tsx` rewritten — `setFocus(fq: FullyQualifiedMoniker | null)` strict. **NEW: `useFocusBySegmentPath()`** helper composes a multi-segment chain under the enclosing primitive's FQM and dispatches setFocus.

### Done in current session — Test infrastructure rewrites

- `kanban-app/ui/src/test/spatial-shadow-registry.ts` — fully rewritten to FQM identity. Map keys are now `FullyQualifiedMoniker`. `ShadowEntry` uses `fq`, `segment`, `layerFq`, `parentZone` (FQM). `fireFocusChanged` accepts `prev_fq`/`next_fq`/`next_segment`. Wire-decoders read `a.fq`/`a.segment`/`a.layerFq`/`a.parentZone`/`a.focusedFq` matching the new IPC shape. New: `getRegisteredFqBySegment` lookup; `setupSpatialHarness` returns FQM-shaped harness.
- `kanban-app/ui/src/test-helpers/kernel-simulator.ts` — fully rewritten. `LayerRecord` has `fq`/`segment`/`name`/`parent`. `RegistrationRecord` keyed by FQM. Command dispatch table reads FQM args. Emit shape `{prev_fq, next_fq, next_segment}`. New: `findBySegment`, `findBySegmentPrefix`, `findByFq` lookups.

### Done in current session — Major production callsite migrations

- `inspectable.tsx` — `Moniker` import → `SegmentMoniker`; signature updates.
- `board-view.tsx` — fully rewired. Action commands moved INSIDE `BoardSpatialZone` (where `useFullyQualifiedMoniker()` returns the board zone FQ). New `BoardSpatialBody` component houses `setFocus`/initial-focus/add-task plumbing. `useInitialFocusTarget` returns `{columnSegment, leafSegment}` so seed focus composes correctly. `focusCreatedTask(taskId, columnSegment)` composes the card FQM under the board-zone FQM.
- `column-view.tsx` — fully rewired. `useStableSpatialKeys` (UUID minting) replaced with `useTaskPlaceholderFqs` (deterministic FQM composition under column FQM). `usePlaceholderRegistration` uses FQM identity. `ColumnHeader`/`AddTaskButton` accept `setFocus(fq: FullyQualifiedMoniker | null)`. `useParentZoneFq` swap. `useOptionalEnclosingLayerFq` swap.
- Bulk `asMoniker → asSegment` in production: `data-table.tsx`, `nav-bar.tsx`, `perspective-container.tsx`, `view-container.tsx`, `mention-view.tsx`, `grid-view.tsx`, `command-palette.tsx`, `entity-card.tsx`, `avatar.tsx`, `fields/field.tsx`, `fields/displays/attachment-display.tsx`, `perspective-tab-bar.tsx`.
- Bulk `useOptionalLayerKey → useOptionalEnclosingLayerFq` import path swap: `data-table.tsx`, `perspective-container.tsx`, `view-container.tsx`, `grid-view.tsx`, `perspective-tab-bar.tsx`.

### Error count progression

- Start of refactor: ~774 errors across ~80 files.
- After this session's work: **~416 errors** remaining (down 358 from start, ~46% reduction).

### NOT done — Section E + remaining migration sweep

The remaining ~416 errors span:

1. **`useCurrentLayerKey` references** — `app-shell.tsx`. Map to `useFullyQualifiedMoniker()`.
2. **`focusedKey()`/`focusedMoniker()` on `SpatialFocusActions` → `focusedFq()`** — `app-shell.tsx`, `fields/field.tsx`, `cursor-focus-bridge.tsx`.
3. **`setFocus` argument typing** — production callers in `cursor-focus-bridge.tsx`, `entity-inspector.tsx` (line 127, 129), `data-table.tsx` (line 986), `grid-view.tsx` (line 268, 767). Each needs FQM composition or signature update; the strategy mirrors what board-view did (compose at the call site under the enclosing primitive's FQM via `useFullyQualifiedMoniker()`).
4. **Tail `setFocus` issues in `focus-scope.tsx`/`focus-zone.tsx`** — passing `SegmentMoniker` where `FullyQualifiedMoniker | null` is expected (lines 486/500 and 598/612). The primitives' fallback behavior dispatches `setFocus(moniker)` outside the spatial-nav stack — the segment cannot be composed without a parent FQ. Either compose via `useOptionalFullyQualifiedMoniker()` and skip the fallback when null, or accept a no-op when no parent FQ.
5. **`focus-scope-context.tsx`** — imports `Moniker` (deleted). Split into `SegmentMoniker` or `FullyQualifiedMoniker` per usage.
6. **Test files** — every `.test.tsx`/`.spec.tsx` that uses old types. Highest-error files:
   - `lib/entity-focus-context.test.tsx` (40)
   - `lib/spatial-focus-context.test.tsx` (29)
   - `spatial-nav-end-to-end.spatial.test.tsx` (27)
   - `components/focus-layer.test.tsx` (24)
   - `components/focus-zone.test.tsx` (21)
   - `components/inspector.kernel-focus-advance.browser.test.tsx` (20)
   - `lib/entity-focus.kernel-projection.test.tsx` (19)
   - `components/inspector-focus-bridge.layer-barrier.browser.test.tsx` (17)
   - `components/inspector.close-restores-focus.browser.test.tsx` (15)
   - `components/inspector.cross-panel-nav.browser.test.tsx` (14)
   - + ~60 more files with 1–10 errors each.
7. **New file `path-monikers.kernel-driven.browser.test.tsx`** — not yet authored. Should host the 7 named tests from parent task `01KQD6064G1C1RAXDFPJVT1F46`.

### Files modified in current session

- `kanban-app/ui/src/components/inspectable.tsx` (Moniker → SegmentMoniker)
- `kanban-app/ui/src/components/fully-qualified-moniker-context.tsx` (added `useChildFq`)
- `kanban-app/ui/src/components/board-view.tsx` (rewrote tail)
- `kanban-app/ui/src/components/column-view.tsx` (rewrote placeholder + header)
- `kanban-app/ui/src/test/spatial-shadow-registry.ts` (full rewrite, FQM identity)
- `kanban-app/ui/src/test-helpers/kernel-simulator.ts` (full rewrite, FQM identity)
- `kanban-app/ui/src/lib/entity-focus-context.tsx` (added `useFocusBySegmentPath`)
- Bulk `asMoniker → asSegment` across production: `avatar.tsx`, `command-palette.tsx`, `data-table.tsx`, `entity-card.tsx`, `fields/field.tsx`, `fields/displays/attachment-display.tsx`, `grid-view.tsx`, `mention-view.tsx`, `nav-bar.tsx`, `perspective-container.tsx`, `perspective-tab-bar.tsx`, `view-container.tsx`.
- Bulk `useOptionalLayerKey` import swap across same group.

### Why the card stays in `doing`

Per the `/implement` skill rules: *"If you cannot complete the task, do NOT move it forward. Add a comment describing what happened and report back."*

The migration sweep across ~80 TS files plus the new browser test file is multi-day mechanical work that does not fit in a single `/implement` pass without context overflow. Each pass makes verifiable progress (this one took 765→416 errors, ~46% reduction). The structural foundation (Sections B/C/D + key production callsites) is in place and architecturally sound.

### Suggested next pass

1. Finish `app-shell.tsx`, `cursor-focus-bridge.tsx`, `entity-inspector.tsx`, `fields/field.tsx`, `data-table.tsx`, `grid-view.tsx`, `focus-scope.tsx`, `focus-zone.tsx`, `focus-scope-context.tsx` (production code).
2. Migrate the 5 named mock files in the task scope.
3. Migrate the wider test sweep — error groups can be addressed file-by-file in parallel.
4. Author `path-monikers.kernel-driven.browser.test.tsx`.

## What

### Tauri command boundary (`kanban-app/src/commands.rs`)

- `spatial_register_scope`/`zone(fq, segment, parent_fq, layer_fq, rect, overrides)` — kernel inserts directly. React composed the FQM.
- `spatial_register_batch` accepts entries with FQM keys.
- `spatial_unregister_scope(fq)`.
- `spatial_focus(fq)`. Delete or alias `spatial_focus_by_moniker`.
- `spatial_navigate(focused_fq, direction)`.
- `spatial_drill_in(fq, focused_fq)`, `spatial_drill_out(fq, focused_fq)`.
- `spatial_clear_focus()` unchanged.
- `spatial_push_layer(fq, segment, parent_fq)`, `spatial_pop_layer(fq)`.

### TS branded types (`kanban-app/ui/src/types/spatial.ts`)

- `SegmentMoniker` and `FullyQualifiedMoniker` distinct branded types.
- `composeFq(parent, segment) -> FullyQualifiedMoniker` utility.
- `FocusChangedPayload` updated to FQM shape.

### React primitives

- `<FocusLayer>`: prop is `name: SegmentMoniker`. Compose FQM via context.
- `<FocusZone>`: prop is `moniker: SegmentMoniker`. Compose FQM via `useFullyQualifiedMoniker()`.
- `<FocusScope>`: same shape.
- `useFullyQualifiedMoniker(): FullyQualifiedMoniker` hook reads from context, throws if no primitive ancestor.

### entity-focus-context

- `setFocus(FullyQualifiedMoniker | null)` strict.
- Bridge subscribes to `focus-changed` (FQM payload).
- `useFocusedSegmentMoniker()` derived.

### Browser tests

- New file `kanban-app/ui/src/components/path-monikers.kernel-driven.browser.test.tsx` with the seven Layer 2 tests from the parent card.
- Update existing browser/spatial tests that used flat monikers/SpatialKeys for `setFocus` callsites.

## Acceptance Criteria

- [x] Tauri commands accept FQM/segment shape.
- [x] `cargo test -p kanban-app` passes.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.
- [x] TS branded types `SegmentMoniker`/`FullyQualifiedMoniker` defined; `SpatialKey`/`LayerKey`/flat `Moniker` removed from `types/spatial.ts`.
- [x] React primitives rewritten.
- [x] `entity-focus-context` rewritten.
- [x] Test infrastructure (spatial-shadow-registry + kernel-simulator) rewritten to FQM identity.
- [x] Major production callsites (board-view, column-view + bulk `asMoniker → asSegment` sweep) migrated.
- [ ] Migration sweep — `npx tsc --noEmit` clean (~416 errors remaining).
- [ ] `bun run test:browser` (and node tests) pass.
- [ ] New file `path-monikers.kernel-driven.browser.test.tsx` with 7 named tests authored and passing.
- [x] `cargo test --workspace` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Out of scope (handled in Layer 3 card)

- `npm run tauri dev` manual log verification.

## Depends on

- Layer 1 sub-task (Rust kernel newtypes).

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
