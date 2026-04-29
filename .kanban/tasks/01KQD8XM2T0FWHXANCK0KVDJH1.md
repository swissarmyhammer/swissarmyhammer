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

## Status (in-progress on this card)

### Done — Section A: Tauri command boundary (`kanban-app/src/commands.rs` + `main.rs`)

- Imports updated to use `FullyQualifiedMoniker` and `SegmentMoniker`; `SpatialKey`, `LayerKey`, `Moniker` removed from kanban-app Rust side.
- All `spatial_*` Tauri commands rewired to FQM/segment shape:
  - `spatial_register_scope(fq, segment, rect, layer_fq, parent_zone, overrides)`
  - `spatial_register_zone(fq, segment, rect, layer_fq, parent_zone, overrides)`
  - `spatial_unregister_scope(fq)`
  - `spatial_update_rect(fq, rect)`
  - `spatial_focus(fq)`
  - `spatial_navigate(focused_fq, direction)`
  - `spatial_drill_in(fq, focused_fq) -> FullyQualifiedMoniker`
  - `spatial_drill_out(fq, focused_fq) -> FullyQualifiedMoniker`
  - `spatial_push_layer(fq, segment, name, parent)`
  - `spatial_pop_layer(fq)`
- `spatial_focus_by_moniker` deleted (the FQM IS the key).
- All `*_inner` helpers, `RegisterEntry` field references, and `spatial_command_tests` module rewritten with FQM API.
- `state.rs` doc comment updated to reference `FullyQualifiedMoniker`.
- `cargo check -p kanban-app` clean.
- `cargo test -p kanban-app` — 93 passed, 0 failed.
- `cargo build --workspace --tests` clean.
- `cargo clippy -p kanban-app --all-targets -- -D warnings` clean.

### Not done — Sections B, C, D, E (TypeScript / React + tests)

Scope: 931 occurrences of `SpatialKey`/`LayerKey`/`Moniker`/`crypto.randomUUID` across 82 TypeScript files, plus authoring new `path-monikers.kernel-driven.browser.test.tsx`.

This is multi-day mechanical work that cannot land in a single `/implement` pass without producing either context overflow or sweeping changes that may not honor the `useFullyQualifiedMoniker()` context-composition intent. Per `/implement` skill rules ("If you cannot complete the task, do NOT move it forward"), Layer 2 has been split: Section A is locked in via the focused commit; Sections B-E should be filed as a follow-up sub-task and worked from there with a fresh `/implement` invocation per layer (e.g. types + primitives in one card; entity-focus-context in a second; test sweep in a third).

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
- [ ] `SpatialKey` and flat `Moniker` types deleted from TS.
- [ ] `setFocus` and `spatial_focus` accept only `FullyQualifiedMoniker`. Segment passed there is a tsc compile error.
- [ ] React consumers declare only `SegmentMoniker` props.
- [x] `cargo test --workspace` passes.
- [ ] `bun run test:browser` (and node tests) pass.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Out of scope (handled in Layer 3 card)

- `npm run tauri dev` manual log verification.

## Depends on

- Layer 1 sub-task (Rust kernel newtypes).

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
