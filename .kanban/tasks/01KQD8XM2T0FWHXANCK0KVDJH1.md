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

## Status — Section A done, Sections B–D scaffolded, E (test sweep + new test file) NOT done

### Done — Section A: Tauri command boundary (`kanban-app/src/commands.rs` + `main.rs`)

(Locked in earlier; cargo build, cargo test -p kanban-app, cargo clippy clean.)

### Done in this session — Section B: TS branded types (`kanban-app/ui/src/types/spatial.ts`)

- New types defined: `SegmentMoniker`, `FullyQualifiedMoniker` (distinct brands), `WindowLabel`, `LayerName`, `Pixels`.
- Helpers: `asSegment`, `asFq`, `asLayerName`, `asWindowLabel`, `asPixels`, `composeFq`, `fqRoot`, `fqLastSegment`.
- Removed: `SpatialKey`, `LayerKey`, flat `Moniker`, `asMoniker`, `asSpatialKey`, `asLayerKey`.
- `FocusChangedPayload` updated: `prev_fq`, `next_fq`, `next_segment` (matching the kernel's emit shape).
- `FocusOverrides` is now `Partial<Record<Direction, FullyQualifiedMoniker | null>>`.
- Companion test file `spatial.test.ts` rewritten with brand-distinctness, `composeFq`, `fqRoot`, `fqLastSegment` coverage.

### Done in this session — Section C: React primitives (`kanban-app/ui/src/components/`)

- New `fully-qualified-moniker-context.tsx` — `FullyQualifiedMonikerContext`, `useFullyQualifiedMoniker`, `useOptionalFullyQualifiedMoniker`.
- New `layer-fq-context.tsx` — `LayerFqContext`, `useEnclosingLayerFq`, `useOptionalEnclosingLayerFq` (broken out to avoid focus-zone ↔ focus-layer cycle).
- `focus-layer.tsx` rewritten — takes `name: SegmentMoniker`, composes own FQM via `fqRoot`/`composeFq`, provides `FullyQualifiedMonikerContext` AND `LayerFqContext`. `crypto.randomUUID()` removed. IPC: `spatial_push_layer({ fq, segment, name, parent })`.
- `focus-zone.tsx` rewritten — `moniker: SegmentMoniker`, composes own FQM, provides `FullyQualifiedMonikerContext` + `FocusZoneContext` (FQM-typed). `useParentZoneFq()` replaces `useParentZoneKey`. IPC: `spatial_register_zone({ fq, segment, rect, layerFq, parentZone, overrides })`. `data-moniker` attr is now the FQM; `data-segment` is the segment.
- `focus-scope.tsx` rewritten with the same shape as FocusZone.
- `use-track-rect-on-ancestor-scroll.ts` updated — takes `FullyQualifiedMoniker` instead of `SpatialKey`.

### Done in this session — Section D: entity-focus-context + spatial-focus-context

- `lib/spatial-focus-context.tsx` rewritten — actions surface takes FQM throughout: `focus(fq)`, `clearFocus()`, `registerScope(fq, segment, rect, layerFq, parentZone, overrides)`, `registerZone(fq, segment, ...)`, `unregisterScope(fq)`, `updateRect(fq, rect)`, `navigate(focusedFq, dir)`, `pushLayer(fq, segment, name, parent)`, `popLayer(fq)`, `drillIn(fq, focusedFq)`, `drillOut(fq, focusedFq)`. New `focusedFq()` replaces `focusedKey()` + `focusedMoniker()`. New `clearFocus` action. `useFocusClaim` takes FQM.
- `lib/entity-focus-context.tsx` rewritten — `setFocus(fq: FullyQualifiedMoniker | null)` strict (segment-form is a TS error). Bridge subscribes to `focus-changed` events with the new `prev_fq`/`next_fq`/`next_segment` shape and writes `next_fq` into the store. New `useFocusedFq()`, `useFocusedSegmentMoniker()`, `useFocusedMonikerRef()` returning FQM. `useFocusedMoniker()` kept as deprecated alias.

### Done in this session — Migrated production callsites

- `App.tsx` — `WINDOW_LAYER_NAME = asSegment("window")`.
- `inspectors-container.tsx` — uses `useFullyQualifiedMoniker()` to read window FQM, passes it as `parentLayerFq` to `<FocusLayer>`. `INSPECTOR_LAYER_NAME = asSegment("inspector")`.
- `board-view.tsx` — partial: `asMoniker` import replaced with `asSegment`, `useOptionalLayerKey` replaced with `useOptionalEnclosingLayerFq`, scroll helper renamed to take `focusedFq`. Body still has migration left.

### NOT done — Sections E + remaining migration sweep

`npx tsc --noEmit` reports **774 errors** across ~80 files. Patterns:

1. **`asMoniker(...)` callsites → `asSegment(...)`** for entity-form strings (`task:T1`, etc.). Production hits: `avatar.tsx`, `board-view.tsx`, `column-view.tsx`, `command-palette.tsx`, `data-table.tsx`, `entity-card.tsx`, `entity-inspector.tsx`, `fields/displays/attachment-display.tsx`, `fields/field.tsx`, `inspectable.tsx`, `mention-view.tsx`, `nav-bar.tsx`, `perspective-tab-bar.tsx`. Plus tests.
2. **`asLayerName(s)` for `<FocusLayer name=>` → `asSegment(s)`** — ~20 test files.
3. **`useOptionalLayerKey()`/`useCurrentLayerKey()` → `useOptionalEnclosingLayerFq()`/`useFullyQualifiedMoniker()`** — `app-shell.tsx`, `board-view.tsx`, `column-view.tsx`, `data-table.tsx`, `perspective-tab-bar.tsx`, etc.
4. **`useParentZoneKey()` → `useParentZoneFq()`** — column-view + data-table.
5. **`focusedKey()`/`focusedMoniker()` on `SpatialFocusActions` → `focusedFq()`** — app-shell.tsx, fields/field.tsx, cursor-focus-bridge.tsx.
6. **`FocusChangedPayload` shape rename** in ~40 test files: `prev_key`→`prev_fq`, `next_key`→`next_fq`, `next_moniker`→`next_segment`.
7. **`setFocus(target: string)` callers** — these need real attention. Today most pass entity-form like `"task:T1"`; FQM requires the full path (`/window/board/column:todo/card:T1`). Patterns:
   - inside the focused primitive: `setFocus(useFullyQualifiedMoniker())`.
   - composing for not-yet-mounted descendant: `setFocus(composeFq(parent, segment))`.
   Hits: `board-view.tsx` (`useInitialBoardFocus`, `useAddTaskHandler`), `app-shell.tsx`, `column-view.tsx`, `fields/field.tsx`, `cursor-focus-bridge.tsx`, `entity-inspector.tsx` (`useFirstFieldFocus` capture/restore), `command-palette.tsx`, keymap path.
8. **`Moniker` type imports** — many files import `type { Moniker }`. Must split into `SegmentMoniker` or `FullyQualifiedMoniker` per usage.
9. **`SpatialKey` references in `test/spatial-shadow-registry.ts`** — `Map<SpatialKey, ShadowEntry>` should become `Map<FullyQualifiedMoniker, ShadowEntry>`. JS port of `BeamNavStrategy` is identifier-agnostic; this is mechanical.
10. **`test-helpers/kernel-simulator.ts`** — same shape: rewrite `LayerRecord`, `RegistrationRecord`, command dispatch table for FQM keys + emit `FocusChangedPayload` with `next_fq`/`prev_fq`/`next_segment`.
11. **The 5 mock files** + **all other test files** that use `setFocus`, `Moniker`, `SpatialKey`, `data-moniker` selectors.
12. **New file `path-monikers.kernel-driven.browser.test.tsx`** — not yet authored. Should host the 7 named tests from parent task `01KQD6064G1C1RAXDFPJVT1F46`.

### Files modified in this session (committable as a structural foundation)

- `kanban-app/ui/src/types/spatial.ts` (rewritten)
- `kanban-app/ui/src/types/spatial.test.ts` (rewritten)
- `kanban-app/ui/src/components/fully-qualified-moniker-context.tsx` (new)
- `kanban-app/ui/src/components/layer-fq-context.tsx` (new)
- `kanban-app/ui/src/components/focus-layer.tsx` (rewritten)
- `kanban-app/ui/src/components/focus-zone.tsx` (rewritten)
- `kanban-app/ui/src/components/focus-scope.tsx` (rewritten)
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` (FQM swap)
- `kanban-app/ui/src/components/inspectors-container.tsx` (parent FQ wiring)
- `kanban-app/ui/src/components/board-view.tsx` (partial)
- `kanban-app/ui/src/App.tsx` (WINDOW_LAYER_NAME swap)
- `kanban-app/ui/src/lib/entity-focus-context.tsx` (rewritten)
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` (rewritten)

### Why the card stays in `doing`

Per the `/implement` skill rules: *"If you cannot complete the task, do NOT move it forward. Add a comment describing what happened and report back."*

The migration sweep across ~80 TS files plus the new browser test file is multi-day mechanical work that does not fit in a single `/implement` pass without context overflow. The structural foundation (Sections B/C/D scaffolding) is in place and architecturally sound; the remaining work is the compile-error wave following through every callsite. The branded types are the safety net — `tsc --noEmit` errors are the worklist.

### Suggested next pass

1. Migrate `test/spatial-shadow-registry.ts` and `test-helpers/kernel-simulator.ts` first (test-helpers many tests transitively depend on).
2. Migrate the production files with `asMoniker` / `useOptionalLayerKey` (board-view, column-view, data-table, app-shell, fields/field, entity-inspector, etc.).
3. Migrate the 5 named mock files in the task scope (app-shell.test.tsx, inspectable.space.browser.test.tsx, grid-view.cursor-ring.test.tsx, board-view.enter-drill-in.browser.test.tsx, entity-inspector.field-enter-drill.browser.test.tsx).
4. Migrate the wider test sweep — error groups can be addressed file-by-file in parallel.
5. Author `path-monikers.kernel-driven.browser.test.tsx`.

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
- Delete the `SpatialKey` and flat `Moniker` brands.

### React primitives

- `<FocusLayer>`: prop is `name: SegmentMoniker`. Compose FQM via context. Provide via `FullyQualifiedMonikerContext.Provider`. `crypto.randomUUID()` removed.
- `<FocusZone>`: prop is `moniker: SegmentMoniker`. Compose FQM via `useFullyQualifiedMoniker()`. Provide composed FQM as the new context for descendants.
- `<FocusScope>`: same shape.
- `useFullyQualifiedMoniker(): FullyQualifiedMoniker` hook reads from context, throws if no primitive ancestor.

### entity-focus-context

- `setFocus(FullyQualifiedMoniker | null)` strict.
- Bridge subscribes to `focus-changed` (FQM payload).
- `useFocusedSegmentMoniker()` derived (last segment of FQM).

### Browser tests

- New file `kanban-app/ui/src/components/path-monikers.kernel-driven.browser.test.tsx` with the seven Layer 2 tests from the parent card.
- Update existing browser/spatial tests that used flat monikers/SpatialKeys for `setFocus` callsites.

## Acceptance Criteria

- [x] Tauri commands accept FQM/segment shape.
- [x] `cargo test -p kanban-app` passes after the Tauri rewire.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.
- [x] TS branded types `SegmentMoniker`/`FullyQualifiedMoniker` defined; `SpatialKey`/`LayerKey`/flat `Moniker` removed from `types/spatial.ts`.
- [x] React primitives rewritten — `<FocusLayer>` / `<FocusZone>` / `<FocusScope>` take `SegmentMoniker` and compose FQM via context; `useFullyQualifiedMoniker()` hook available.
- [x] `entity-focus-context` rewritten — `setFocus` takes `FullyQualifiedMoniker | null` strictly; bridge writes `next_fq` into store; `useFocusedSegmentMoniker` available.
- [ ] Migration sweep — `npx tsc --noEmit` clean across ~80 callsites (~774 errors remaining).
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
