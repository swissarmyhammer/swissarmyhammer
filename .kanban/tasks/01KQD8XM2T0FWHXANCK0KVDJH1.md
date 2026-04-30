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

## Status — Sections A-D + production-callsite migration done; TS compile clean; vitest semantic fixes ongoing

### Done — Section A: Tauri command boundary (`kanban-app/src/commands.rs` + `main.rs`)

(Locked in earlier; cargo build, cargo test -p kanban-app, cargo clippy clean.)

### Done — Section B: TS branded types (`kanban-app/ui/src/types/spatial.ts`)

- New types defined: `SegmentMoniker`, `FullyQualifiedMoniker` (distinct brands), `WindowLabel`, `LayerName`, `Pixels`.
- Helpers: `asSegment`, `asFq`, `asLayerName`, `asWindowLabel`, `asPixels`, `composeFq`, `fqRoot`, `fqLastSegment`.
- Removed: `SpatialKey`, `LayerKey`, flat `Moniker`, `asMoniker`, `asSpatialKey`, `asLayerKey`.
- `FocusChangedPayload` updated: `prev_fq`, `next_fq`, `next_segment`.

### Done — Section C: React primitives

- `fully-qualified-moniker-context.tsx` — `FullyQualifiedMonikerContext`, `useFullyQualifiedMoniker`, `useOptionalFullyQualifiedMoniker`, `useChildFq(segment)` convenience helper.
- `layer-fq-context.tsx` — `LayerFqContext`, `useEnclosingLayerFq`, `useOptionalEnclosingLayerFq`.
- `focus-layer.tsx`, `focus-zone.tsx`, `focus-scope.tsx` rewritten — take `SegmentMoniker`, compose own FQM via context.
- `use-track-rect-on-ancestor-scroll.ts` updated.

### Done — Section D: spatial-focus + entity-focus contexts

- `lib/spatial-focus-context.tsx` rewritten — actions surface takes FQM.
- `lib/entity-focus-context.tsx` rewritten — `setFocus(fq: FullyQualifiedMoniker | null)` strict. `useFocusBySegmentPath()` helper.

### Done — Test infrastructure rewrites

- `kanban-app/ui/src/test/spatial-shadow-registry.ts` — fully rewritten to FQM identity.
- `kanban-app/ui/src/test-helpers/kernel-simulator.ts` — fully rewritten to FQM identity.

### Done — Production callsite migration

- All production .tsx/.ts files now compile clean against the new types:
  - `app-shell.tsx` — `useEnclosingLayerFq`, `focusedFq()`, FQM-typed setFocus refs, `parentLayerFq` prop, `asSegment("palette")`.
  - `board-view.tsx` — fully rewired (BoardSpatialZone composing FQs internally).
  - `column-view.tsx` — fully rewired (FQM placeholder registration).
  - `data-table.tsx` — `EntityRow` reads `useFullyQualifiedMoniker()` for setFocus dispatch.
  - `entity-inspector.tsx` — `useFirstFieldFocus` takes FQM; composed under inspector layer FQ.
  - `fields/field.tsx` — drillIn uses `focusedFq()` from spatial actions.
  - `grid-view.tsx` — `useGridNavigation` uses `useFocusBySegmentPath` adapter for cell focus.
  - `cursor-focus-bridge.tsx` — typed FQM.
  - `focus-zone.tsx`, `focus-scope.tsx` — fallback paths compose FQM via `useOptionalFullyQualifiedMoniker` and skip dispatch if no parent.
  - `focus-scope-context.tsx` — `FocusScopeContext` typed `FullyQualifiedMoniker | null`.

### Done — TypeScript compile is CLEAN

- `cd kanban-app/ui && npx tsc --noEmit` returns 0 errors (was 808 at session start, 416 carryover from previous pass).

### Done — Bulk test file migration

Mass sed/perl across all 130+ test files:
- `asMoniker` → `asSegment`, `asLayerName` → `asSegment`, `asSpatialKey`/`asLayerKey` → `asFq`, `asFullyQualifiedMoniker` → `asFq`.
- `SpatialKey`/`LayerKey` → `FullyQualifiedMoniker`, `Moniker` → `SegmentMoniker`.
- `next_key`/`prev_key` → `next_fq`/`prev_fq`, `next_moniker` → `next_segment`.
- `findByMoniker`/`findByMonikerPrefix` → `findBySegment`/`findBySegmentPrefix`.
- `getRegisteredKeyByMoniker` → `getRegisteredFqBySegment`.
- `useOptionalLayerKey` → `useOptionalEnclosingLayerFq`, `useCurrentLayerKey` → `useEnclosingLayerFq`, `useParentZoneKey` → `useParentZoneFq`.
- `actions.focusedKey()` → `actions.focusedFq()`.
- `EntityFocusContextValue.focusedMoniker` → `focusedFq` (destructure-aware).
- `RegistrationRecord.key/moniker` → `.fq/.segment` (precise per-error patches).
- `parentLayerKey=` → `parentLayerFq=`.
- `currentFocus.key` → `currentFocus.fq`.
- Many `setFocus("..")` → `setFocus(asFq(".."))` wrap-up at error sites.
- `spatial_register_*` arg shape helpers in board/column tests rewritten with new `fq`/`segment`/`layerFq` field names.

### NOT done — vitest semantic test failures

- `npx vitest run` still has **59 failed test files / 278 failed tests** of 178/1768 total.
- Root cause: many tests use `mockInvoke.mock.calls` to assert on the *runtime* IPC arg shape — they spelled `args.key`/`args.moniker`/`args.layerKey`/`args.parent` which now ship as `args.fq`/`args.segment`/`args.layerFq`. Some of these were caught by the typed-helper rewrites, but many tests do dynamic property access (`Record<string, unknown>`) and slip through TypeScript.
- Each failing test file needs its `mockInvoke.mock.calls` accessors and assertion property names migrated. Pattern: change `(c) => c[1] as { key: string; moniker: string; layerKey: string }` to `(c) => c[1] as { fq: string; segment: string; layerFq: string }`, and update downstream `.key`/`.moniker`/`.layerKey` accesses.
- Also: many `next_segment: "..."` strings need `asSegment("...")` wrapping; `setFocus(rawString)` calls need `asFq(...)` wrapping.

### NOT done — New test file `path-monikers.kernel-driven.browser.test.tsx`

The 7 named tests from parent `01KQD6064G1C1RAXDFPJVT1F46` have not yet been authored.

### Error count progression

- Start of refactor: ~774 errors across ~80 files.
- End of previous pass: 416 errors.
- End of this pass: **0 TS errors** (clean).
- vitest: 278 failures pending semantic migration.

### Suggested next pass

1. Fix the runtime IPC arg-shape assertions across remaining 59 test files. Each file roughly:
   - Find `as { key:.*moniker:.*layerKey:` literal types — rewrite to FQM shape.
   - Find `(args ?? {}) as { key?: .. moniker?: .. }` — same.
   - Find body uses of `a.key`/`a.moniker`/`a.layerKey`/`a.parent` and rewrite to `a.fq`/`a.segment`/`a.layerFq`/(layer parent stays `parent`).
   - Fix `setFocus("string")` → `setFocus(asFq("string"))` where types loosen.
2. Author `path-monikers.kernel-driven.browser.test.tsx` with the 7 named tests.
3. Move task to review.

## Why the card stays in `doing`

The TS migration is structurally complete — every production callsite compiles against the path-monikers identity model, and all type-level tests pass. The remaining work is mechanical wire-shape updates in vitest assertions. That work is parallelizable and has a clear pattern; a follow-on pass can knock it out file-by-file.

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
- [x] All production code migrated — `npx tsc --noEmit` is **zero errors**.
- [ ] `bun run test:browser` (and node tests) pass — 278 vitest failures pending semantic IPC arg-shape migration in test files.
- [ ] New file `path-monikers.kernel-driven.browser.test.tsx` with 7 named tests authored and passing.
- [x] `cargo test --workspace` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Out of scope (handled in Layer 3 card)

- `npm run tauri dev` manual log verification.

## Depends on

- Layer 1 sub-task (Rust kernel newtypes).

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
