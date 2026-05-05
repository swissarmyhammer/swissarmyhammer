---
assignees:
- wballard
depends_on:
- 01KQW69GDFYZ1QYV9TMBD5F9RR
- 01KQW6BJZ6DTZSHKKEDP5TEG4E
- 01KQW6D6B2JXPA4PX6H94R86KB
position_column: todo
position_ordinal: d880
project: spatial-nav
title: 'spatial-nav redesign step 9: dev-mode dual-source soak — verify zero divergence under real use'
---
## Parent

Implementation step for **01KQTC1VNQM9KC90S65P7QX9N1**.

## Goal

Bake-in period. Run the snapshot path and the registry path side-by-side in dev builds for a sustained period, capture every divergence, and fix the underlying bug. This is the gate before cutover (steps 10–13).

## What to build

### Divergence harness consolidation

Steps 6–8 each added their own divergence diagnostic. Consolidate into a single dev-mode harness:

```rust
#[cfg(debug_assertions)]
fn compare_paths<R, F1, F2>(op: &str, snapshot_path: F1, registry_path: F2) -> R
where
    R: PartialEq + std::fmt::Debug,
    F1: FnOnce() -> R,
    F2: FnOnce() -> R,
{
    let snapshot_result = snapshot_path();
    let registry_result = registry_path();
    if snapshot_result != registry_result {
        tracing::warn!(
            op = %op,
            snapshot = ?snapshot_result,
            registry = ?registry_result,
            "spatial-nav snapshot/registry divergence",
        );
    }
    snapshot_result // snapshot is the future-authoritative source
}
```

Wire `compare_paths` into the three dual-running call sites: `spatial_navigate`, `spatial_focus`, `spatial_focus_lost`. In release builds, only the snapshot path runs.

### Soak tests (CI)

Long-running integration tests that mount the kanban-board harness and exercise every nav scenario the production app uses:

- Arrow nav across all four directions from every column position
- Click focus on every scope type (chip, field, button, card, column header)
- Drag-drop a card between columns; nav from the moved card
- Filter changes that hide the focused row; assert focus restoration
- Layer push (open inspector) → nav inside → layer pop → focus restored
- Modal dialog push → focus inside → cancel → focus restored
- Bulk actions that delete multiple cards including the focused one

Each test runs with the divergence harness active and asserts zero `tracing::warn!` events with the divergence message. Use a `tracing::subscriber::test::with_default` to capture warnings.

### Manual soak protocol

The implementer should also run the kanban app in dev mode (`pnpm tauri dev`) and exercise the UI for at least an hour, watching `just logs | grep "snapshot/registry divergence"` for any production-only divergences not caught by automated tests.

### Bug-fix loop

Every captured divergence is an architectural defect — the snapshot path doesn't yet match the registry path for some scenario. Fix at the appropriate layer (steps 3, 4, 5 most likely) and re-run.

## Acceptance criteria

- Soak test suite runs all nav scenarios with zero divergence warnings
- Manual soak in dev mode for ≥1 hour produces zero divergence warnings
- Any divergence found gets a regression test added to the soak suite

## Out of scope

- Cutover (steps 10–13) starts only after this step is fully green

## Files

- New `swissarmyhammer-focus/src/divergence.rs` (the harness)
- `kanban-app/src/commands.rs` — wrap snapshot/registry calls in `compare_paths`
- New `kanban-app/ui/src/spatial-nav-soak.spatial.test.tsx` (integration test bundle) #01KQTC1VNQM9KC90S65P7QX9N1