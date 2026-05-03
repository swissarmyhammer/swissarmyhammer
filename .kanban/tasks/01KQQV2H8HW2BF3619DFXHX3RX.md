---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: todo
position_ordinal: d480
project: spatial-nav
title: 'Spatial-nav #6: coordinate consistency — TS audit + kernel debug assertions'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting, especially the "Coordinate system" invariant.

**This component owns:** verifying and enforcing the coordinate-system invariant that makes the geometric algorithm correct.

**Why it's load-bearing:** geometric pick (component #1) is correct *iff* all candidate rects in the same layer were sampled in the same coordinate system. If some scopes register viewport-relative rects and others register document-relative rects, or if some rects are stale (sampled before a scroll), geometric distance is meaningless and the kernel produces wrong answers silently. No exceptions, no warnings — just bad nav.

**Contract (restated from design):**

> All registered rects are viewport-relative, sampled by `getBoundingClientRect()`, and refreshed on ancestor scroll via `useTrackRectOnAncestorScroll`. The kernel's geometric pick is correct iff this invariant holds across all candidate rects in the same layer.

## What

### Files to audit / modify

- **TypeScript audit (read-only first)**: enumerate every callsite that calls `spatial_register_scope`, `spatial_register_zone`, or `spatial_update_rect`. Confirm each one passes a rect derived from `getBoundingClientRect()` on the scope's own DOM element, not from a parent's rect or a computed offset.
  - `kanban-app/ui/src/components/focus-scope.tsx::SpatialFocusScopeBody` — the registration in `useEffect` (line ~342) calls `node.getBoundingClientRect()` and passes pixels. ✓
  - `kanban-app/ui/src/components/focus-zone.tsx::SpatialFocusZoneBody` — same pattern (line ~404). ✓
  - `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — confirm it calls `node.getBoundingClientRect()` on rect refresh, NOT a cached value or a computed delta.
  - `kanban-app/ui/src/lib/spatial-focus-context.tsx` — confirm the IPC adapters pass rects through unchanged (no double-conversion, no offset application).
  - Document the audit results in the PR description: list every callsite, confirm each one is correct or fix it.

- **Add a TS-side dev-mode validator** (new file `kanban-app/ui/src/lib/rect-validation.ts`):
  - Wrap each registration call in dev mode (`process.env.NODE_ENV === 'development'`).
  - Validate: rect has positive width and height (>0), rect's coordinates are finite (not NaN/Infinity), rect was sampled within the last animation frame (timestamp the call).
  - On violation, `console.error` with the FQM, the rect, and the offending property. Don't throw — log and continue.

- **Kernel-side debug assertions** in `swissarmyhammer-focus/src/registry.rs::register_*`:
  - In `cfg(debug_assertions)` blocks, validate registered rects: positive dimensions, finite coordinates, not absurdly large (>1e6 pixels suggests a unit error or document-relative coords).
  - On violation, `tracing::error!` with the rect and FQM. Don't panic — log and continue (the registry is best-effort).

- **Kernel-side coordinate-system smoke check** in `swissarmyhammer-focus/src/registry.rs`:
  - Add a debug-only `validate_coordinate_consistency(layer)` method: walk all registered scopes in the layer, compute the centroid of all rects, log a warning if any scope's rect is more than (say) 10× the median rect distance from the centroid — strong signal of a coordinate-system mismatch.
  - Call it lazily (e.g. on first nav per layer) to avoid per-registration cost.

- `swissarmyhammer-focus/README.md`:
  - Add a "## Coordinate system" section. Document: viewport-relative, sampled by `getBoundingClientRect`, refreshed on scroll, the dev-mode validators on both sides. Cross-reference this task ID for the audit history.

### Tests

- **Unit tests in `kanban-app/ui/src/lib/rect-validation.test.ts`**:
  - Valid rect → no error.
  - Negative width → error logged.
  - NaN coordinate → error logged.
  - Stale timestamp (>16ms old) → warning logged.

- **Unit tests in `swissarmyhammer-focus/src/registry.rs::tests`** (`#[cfg(debug_assertions)]`):
  - Register a rect with negative width → `tracing::error!` fires (use `tracing-test` or capture).
  - Register a rect with infinite y → error fires.
  - Register a sane rect → no error.
  - `validate_coordinate_consistency` with all-similar rects → no warning.
  - With one rect 10000× further from centroid than the rest → warning fires.

- **Integration test in `swissarmyhammer-focus/tests/coordinate_invariants.rs`** (new):
  - Build a layer with a mix of registered scopes whose rects are intentionally inconsistent (e.g. half viewport-relative, half document-relative). Drive a few cardinal nav calls. Assert that even with bad input, the algorithm doesn't panic and returns *some* FQM (no-silent-dropout still holds even with bad input).

- Run `cargo test -p swissarmyhammer-focus coordinate_invariants` and `pnpm -C kanban-app/ui test rect-validation` and confirm green.

## Acceptance Criteria

- [ ] PR description contains the audit: every registration callsite enumerated, with a ✓ or a "fixed in this PR" note.
- [ ] Dev-mode TS validator wraps every registration; logs `console.error` on bad rects without throwing.
- [ ] Kernel-side `cfg(debug_assertions)` validators in `register_*` log on bad rects.
- [ ] `validate_coordinate_consistency` flags layers with rects in mixed coordinate systems.
- [ ] No panics or assertion failures on bad input — best-effort validation, observability only.
- [ ] README "## Coordinate system" section captures the invariant and the validators.
- [ ] `cargo test -p swissarmyhammer-focus` and `pnpm -C kanban-app/ui test` pass.

## Workflow

- Can run **in parallel with #1** — this task is observability and validation, not algorithm change.
- Start with the read-only audit. If it surfaces a bug, fix it as part of this task and document the bug in the PR description.
- Use `/tdd` for the validators: write the failing-input tests first, then implement.
#spatial-nav-redesign