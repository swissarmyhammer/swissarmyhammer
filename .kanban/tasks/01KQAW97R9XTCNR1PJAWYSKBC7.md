---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffdc80
project: spatial-nav
title: Spatial-nav kernel returns the focused moniker on "no motion" — eliminate Option&lt;Moniker&gt; from nav/drill APIs
---
## What

Replace the kernel's `Option<Moniker>` return shape on `BeamNavStrategy::next`, `SpatialRegistry::drill_in`, and `SpatialRegistry::drill_out` with a non-optional `Moniker`. When motion is not possible, the kernel **returns the focused entry's own moniker** rather than `None`. When the kernel hits a torn-state / unknown-key path, it **emits a `tracing::error!`** with enough context to debug AND echoes back the input moniker so the React side has a valid result. The React side never sees a silent "kernel had no answer" state; every nav / drill dispatch produces an observable focus state, and every actual error is visible in logs.

## Status: IMPLEMENTED

All acceptance criteria met. Implementation summary:

### Kernel contract
- [x] `BeamNavStrategy::next` returns `Moniker` (was `Option<Moniker>`).
- [x] `SpatialRegistry::drill_in` returns `Moniker` (was `Option<Moniker>`).
- [x] `SpatialRegistry::drill_out` returns `Moniker` (was `Option<Moniker>`).
- [x] All three APIs accept a `focused_moniker: &Moniker` parameter alongside the SpatialKey.

### Cardinal nav (BeamNavStrategy::next)
- [x] Layer-root edge → echoes focused_moniker, no trace.
- [x] Override wall → echoes focused_moniker, no trace.
- [x] Torn parent ref → echoes focused_moniker AND traces `tracing::error!` with op="nav".
- [x] Unknown focused key → echoes focused_moniker AND traces.
- [x] Normal cascade → returns the new focus moniker as today.

### Drill-in (SpatialRegistry::drill_in)
- [x] Zone with children → returns first/remembered child's moniker.
- [x] Zone with no children → echoes focused_moniker, no trace.
- [x] Leaf focused → echoes focused_moniker, no trace.
- [x] Unknown key → echoes focused_moniker AND traces.

### Drill-out (SpatialRegistry::drill_out)
- [x] Scope with parent zone → returns parent's moniker.
- [x] Layer-root scope → echoes focused_moniker, no trace.
- [x] Torn parent ref → echoes focused_moniker AND traces.
- [x] Unknown key → echoes focused_moniker AND traces.

### React-side semantics preserved
- [x] `nav.drillOut` dispatches `app.dismiss` when result === focusedMoniker.
- [x] `field.edit` enters edit mode when drillIn returns focused moniker.
- [x] `field.edit` drills into pills when drillIn returns a different moniker.
- [x] No null/undefined focus blip — `setFocus` is idempotent on identity-stable monikers.

### No silent dropouts
- [x] Every `Option<Moniker>` removed from public nav/drill API surface.
- [x] Every torn-state path emits `tracing::error!` with op discriminator.

## Files changed

### Rust kernel
- `swissarmyhammer-focus/src/lib.rs` — top-of-crate docstring documents the no-silent-dropout contract.
- `swissarmyhammer-focus/src/navigate.rs` — `NavStrategy::next` now returns `Moniker`; `BeamNavStrategy::next` echoes focused_moniker on torn state with tracing; new `ParentResolution` enum distinguishes layer-root edge (silent) from torn state (traced).
- `swissarmyhammer-focus/src/registry.rs` — `drill_in`/`drill_out` now return `Moniker`; trace on unknown key and orphan parent ref.
- `swissarmyhammer-focus/src/state.rs` — `SpatialState::navigate_with` reads focused_moniker from registry and threads it through.
- `swissarmyhammer-focus/Cargo.toml` — added `tracing-subscriber` as dev-dependency for the capture layer.

### Tests
- `swissarmyhammer-focus/tests/no_silent_none.rs` (new, 10 tests) — pins the contract: each path returns the right moniker AND emits the right number of trace events.
- `swissarmyhammer-focus/tests/drill.rs` — every assertion updated to the new contract.
- `swissarmyhammer-focus/tests/navigate.rs` — every None assertion updated.
- `swissarmyhammer-focus/tests/inspector_dismiss.rs` — every drill_out call updated.
- `swissarmyhammer-focus/tests/unified_trajectories.rs`, `overrides.rs`, `card_directional_nav.rs`, `navbar_arrow_nav.rs`, `perspective_bar_arrow_nav.rs`, `traits_object_safe.rs` — nav helpers and assertions updated.

### Tauri commands
- `kanban-app/src/commands.rs` — `spatial_drill_in`/`spatial_drill_out` take `focused_moniker: Moniker` and return `Moniker` (was `Option<Moniker>`).

### React side
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — `drillIn`/`drillOut` take `focusedMoniker` parameter, return `Promise<Moniker>`; new `focusedMoniker()` action mirrors `focusedKey()`.
- `kanban-app/ui/src/lib/spatial-focus-context.test.tsx` — 4 tests updated to the new contract shape.
- `kanban-app/ui/src/components/app-shell.tsx` — `buildDrillCommands` reads focusedMoniker, compares result against it: drill-in dispatches `setFocus(result)` unconditionally (idempotent on equality); drill-out dispatches `app.dismiss` on equality, `setFocus(result)` otherwise.
- `kanban-app/ui/src/components/app-shell.test.tsx` — 2 tests updated to assert on the new "echo focused moniker" contract.
- `kanban-app/ui/src/components/fields/field.tsx` — `field.edit` execute closure compares drillIn result against focused moniker; equal → onEdit, different → setFocus.
- `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`, `entity-inspector.field-enter-drill.browser.test.tsx`, `field.enter-edit.browser.test.tsx` — invoke mocks updated to mirror the new echo contract.

## Test results

- `cargo test -p swissarmyhammer-focus`: 161 tests pass (10 new in no_silent_none.rs).
- `cargo test -p kanban-app`: 93 tests pass.
- `cargo build --workspace --tests`: clean.
- `cargo clippy -p swissarmyhammer-focus --all-targets`: clean.
- `cargo clippy -p kanban-app --all-targets`: clean.
- `npx tsc --noEmit` (UI): clean.
- `npx vitest run`: 1838 passed | 1 skipped (1839 total).