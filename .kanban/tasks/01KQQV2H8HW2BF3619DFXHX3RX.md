---
assignees:
- claude-code
depends_on:
- 01KQQSXM2PEYR1WAQ7QXW3B8ME
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffff8380
project: spatial-nav
title: 'Spatial-nav #6: coordinate consistency — TS audit + kernel debug assertions'
---
## Reference

Part of the spatial-nav redesign. Full design: **`01KQQSXM2PEYR1WAQ7QXW3B8ME`** — read it before starting, especially the "Coordinate system" invariant.

**This component owns:** verifying and enforcing the coordinate-system invariant that makes the geometric algorithm correct.

**Why it's load-bearing:** geometric pick (component #1) is correct *iff* all candidate rects in the same layer were sampled in the same coordinate system. If some scopes register viewport-relative rects and others register document-relative rects, or if some rects are stale (sampled before a scroll), geometric distance is meaningless and the kernel produces wrong answers silently. No exceptions, no warnings — just bad nav.

**Contract (restated from design):**

> All registered rects are viewport-relative, sampled by `getBoundingClientRect()`, and refreshed on ancestor scroll via `useTrackRectOnAncestorScroll`. The kernel's geometric pick is correct iff this invariant holds across all candidate rects in the same layer.

## Audit

Every TS-side spatial-nav registration callsite enumerated. All sample rects via `node.getBoundingClientRect()` directly on the scope's own DOM element — no cached values, no parent rects, no computed offsets:

- `kanban-app/ui/src/components/focus-scope.tsx::SpatialFocusScopeBody` — registration `useEffect` calls `node.getBoundingClientRect()`. ✓
- `kanban-app/ui/src/components/focus-scope.tsx` — ResizeObserver-driven `updateRect` calls `node.getBoundingClientRect()`. ✓
- `kanban-app/ui/src/components/focus-zone.tsx::SpatialFocusZoneBody` — registration `useEffect` calls `node.getBoundingClientRect()`. ✓
- `kanban-app/ui/src/components/focus-zone.tsx` — ResizeObserver-driven `updateRect` calls `node.getBoundingClientRect()`. ✓
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — rAF-throttled scroll listener calls `live.getBoundingClientRect()`. ✓
- `kanban-app/ui/src/lib/spatial-focus-context.tsx` — IPC adapters (`registerScope`, `registerZone`, `updateRect`) pass rects through unchanged; no double-conversion, no offset application. ✓
- `kanban-app/ui/src/components/column-view.tsx` — placeholder `spatial_register_batch` for off-screen virtualizer rows uses `scrollEl.getBoundingClientRect()` baseline plus `i * H - scrollOffset`; that subtraction matches the viewport-relative frame `getBoundingClientRect()` produces for real-mounted siblings. ✓

No corrections were needed; the audit confirmed every callsite is correct.

## What was added

### TypeScript dev-mode validator

`kanban-app/ui/src/lib/rect-validation.ts` (new):

- Pure `validateRect(op, rect, sampledAtMs, nowMs?)` — returns `{ errors, warnings, preLayoutTransient }` for finite, positive-dim, plausible-scale, and stale-timestamp checks. The `op` argument distinguishes the registration path (`register_scope` / `register_zone`) from `update_rect` so a zero dim on registration is treated as a pre-layout transient (warning) rather than an error.
- `validateAndLogRect(op, fq, rect, sampledAtMs, enabled?)` — logs each error via `console.error`, each warning via `console.warn`, with structured tags. No-op when `enabled` is false. Default `enabled` reads `import.meta.env.DEV`. Zero-dim emissions are deduped per `(op, fq)` so a re-mounting scope or rapid ResizeObserver burst does not flood the channel.
- `__resetPreLayoutTransientLog()` — test-only reset for the dedup set.
- `isDevModeRectValidationEnabled()` — gating helper.

Wired into `kanban-app/ui/src/lib/spatial-focus-context.tsx`'s three rect-bearing IPC adapters: `registerScope`, `registerZone`, `updateRect`. Each adapter accepts an optional `sampledAtMs: number` parameter; the producing callsite captures `performance.now()` immediately after `getBoundingClientRect()` and threads it through. Legacy callers that omit the timestamp fall back to the adapter-boundary `performance.now()` (the staleness check becomes a no-op for them but the rest of the validator still fires).

### Sample-time threading (review fix)

The original wiring captured `sampledAtMs` at the validator boundary, on the same tick as `nowMs` — making the staleness check a no-op. Threading fixed:

- `SpatialFocusActions.registerScope` / `registerZone` / `updateRect` signatures extended with optional `sampledAtMs?: number`.
- `kanban-app/ui/src/components/focus-scope.tsx` — captures `performance.now()` adjacent to each `getBoundingClientRect()` call (initial register, ResizeObserver) and threads it.
- `kanban-app/ui/src/components/focus-zone.tsx` — same threading for the zone register + ResizeObserver.
- `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts` — captures `sampledAtMs` inside the rAF callback and threads it through `updateRect`. Updated the local `UpdateRect` type to accept the optional third arg.

### Kernel-side debug assertions

`swissarmyhammer-focus/src/registry.rs::validate_rect_invariants` (`cfg(debug_assertions)` gated body, no-op release): emits `tracing::error!` per finite / negative-dim / plausible-scale violation, tagged with op + FQM. On the registration ops, a zero dim is treated as a pre-layout transient and surfaces as `tracing::warn!` (mirrors the TS-side behaviour); on `update_rect`, a zero dim stays in the error path because layout has already run by the time ResizeObserver / scroll fires. Called from `register_scope`, `register_zone`, and `update_rect`.

### Coordinate-consistency walk

`SpatialRegistry::validate_coordinate_consistency(layer_fq)` (public method): walks every scope in the layer, computes the **median** rect-center position (robust to outliers in a way the mean is not — see in-source comment for the upper-middle vs textbook-average median note), and emits one `tracing::warn!` per scope whose distance to that position is more than 10× the median distance. Backed by a `validated_layers: HashSet<FullyQualifiedMoniker>` cache so the walk is paid for once per layer per session; the cache is invalidated by `register_scope`, `register_zone`, `update_rect`, `unregister_scope`, and `remove_layer`. `push_layer` is intentionally NOT an invalidator (documented in the field's docstring) because re-pushing a layer does not move any scope rects.

### Tests

- `kanban-app/ui/src/lib/rect-validation.test.ts` — 27 unit tests covering valid / negative / NaN / Infinity / stale / plausible-scale / pre-layout transient (registration) / post-layout zero-dim (update_rect) / dedup behaviour / dev-mode gate. All pass.
- `swissarmyhammer-focus/src/registry.rs::tests` — 9 unit tests covering the kernel-side validator (negative width, infinite y, plausible scale, sane rect, zero-dim registration warning, both-zero-on-registration warning, zero-dim on update_rect error, `update_rect` propagation, consistency-walk uniform/outlier, lazy cache). All pass.
- `swissarmyhammer-focus/tests/coordinate_invariants.rs` — 3 integration tests confirming nav still produces a valid FQM in the same layer when the registry is fed mixed-coord rects, that bad-rect registrations don't panic, and that `validate_coordinate_consistency` is observability-only. All pass.

### README

`swissarmyhammer-focus/README.md` — `## Coordinate system` section documenting the contract (viewport-relative, `getBoundingClientRect`, refreshed on scroll), the TS-side and kernel-side validators, the lazy consistency walk, and a cross-reference to this task ID for the audit history. Added a cross-reference for `tests/coordinate_invariants.rs`.

## Test status (post-review-fixes)

- `cargo test -p swissarmyhammer-focus --lib` — 37 passed, 0 failed.
- `cargo test -p swissarmyhammer-focus --test coordinate_invariants` — 3 passed, 0 failed.
- `cargo nextest run -p swissarmyhammer-focus` — 251 passed, 0 skipped.
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` — clean.
- `pnpm -C kanban-app/ui test rect-validation` — 27 passed, 0 failed.
- `pnpm -C kanban-app/ui exec vitest run` — 1983 passed, 1 skipped, 0 failed (full vitest suite).
- `pnpm -C kanban-app/ui exec tsc --noEmit` — clean.
- Vitest noise from rect-validation: 2000 errors → 137 errors (93% reduction) and 223 warnings (one per distinct (op, fq) zero-dim emission), down from per-component error spam.

## Acceptance Criteria

- [x] PR description contains the audit: every registration callsite enumerated, with a ✓ or a "fixed in this PR" note.
- [x] Dev-mode TS validator wraps every registration; logs `console.error` on bad rects without throwing.
- [x] Kernel-side `cfg(debug_assertions)` validators in `register_*` log on bad rects.
- [x] `validate_coordinate_consistency` flags layers with rects in mixed coordinate systems.
- [x] No panics or assertion failures on bad input — best-effort validation, observability only.
- [x] README "## Coordinate system" section captures the invariant and the validators.
- [x] `cargo test -p swissarmyhammer-focus` (lib + new test files) and `pnpm -C kanban-app/ui test rect-validation` pass.

## Workflow

- Can run **in parallel with #1** — this task is observability and validation, not algorithm change.
- Start with the read-only audit. If it surfaces a bug, fix it as part of this task and document the bug in the PR description.
- Use `/tdd` for the validators: write the failing-input tests first, then implement.
#spatial-nav-redesign

## Review Findings (2026-05-03 18:27)

### Warnings

- [x] `kanban-app/ui/src/lib/spatial-focus-context.tsx` (`registerScope`, `registerZone`, `updateRect` adapters) — staleness check is effectively dead in the current wiring. **FIXED**: extended `SpatialFocusActions.registerScope` / `registerZone` / `updateRect` signatures with an optional `sampledAtMs?: number` parameter. Each producing callsite (`focus-scope.tsx`, `focus-zone.tsx`, `use-track-rect-on-ancestor-scroll.ts`) now captures `performance.now()` immediately after `getBoundingClientRect()` and threads it through. The validator's `nowMs` (captured inside `validateRect` via the default `performance.now()` arg) is now meaningfully later than `sampledAtMs` for any rect that ages between sample and IPC dispatch — restoring the staleness check to a working state. Legacy callers without the argument fall back to the adapter-boundary `performance.now()` (a no-op for the staleness check, but the other validators still fire).

- [x] `kanban-app/ui/src/lib/rect-validation.ts` `pushPositiveDimensionErrors` (and the kernel-side mirror in `swissarmyhammer-focus/src/registry.rs::validate_rect_invariants`) — the `width <= 0` / `height <= 0` check is too tight for legitimate transient state. **FIXED**: the validator now distinguishes registration ops (`register_scope` / `register_zone`) from `update_rect`. On registration, a zero in either dimension is treated as a pre-layout transient and surfaces as a one-shot `console.warn` per `(op, fq)`; on `update_rect` (which fires post-layout), zero dims stay in the error channel but are also deduped per `(op, fq)` so a test-mode burst doesn't flood the console. Negative dims always stay in the error path (negative dims are not what `getBoundingClientRect()` produces for an unlaid-out node). The vitest noise dropped from "many per-component errors" to ~137 deduped errors and 223 deduped warnings across the full 1983-test suite.

### Nits

- [x] `swissarmyhammer-focus/src/registry.rs::validate_coordinate_consistency` median computation — **FIXED**: added an in-source comment at the median computation explaining that this is the "approximate / quickselect-style" median (upper-middle for even-sized arrays) rather than the textbook `(xs[n/2 - 1] + xs[n/2]) / 2` average-of-two-middles. The off-by-half-a-rect difference is irrelevant against the 10× outlier multiplier; the comment notes the convention applies to `ys` and `sorted_distances` too.

- [x] `kanban-app/ui/src/lib/rect-validation.test.ts` (`collects errors for multiple bad components on one rect`) — **FIXED**: extended the inline comment to include "(the plausible-scale check skips non-finite values too)" so the arithmetic is fully traceable. The total is exactly 4 because the plausible-scale check skips non-finite values for both `x`/`y` and the dim errors fire because `-1` is finite.

- [x] `swissarmyhammer-focus/src/registry.rs::push_layer` does not invalidate `validated_layers` — **FIXED**: extended the docstring on the `validated_layers` field to spell out that `push_layer` is intentionally not an invalidator: re-pushing a layer (StrictMode double-mount, palette open/close cycles, IPC re-batch) does not move any scope rects, so the cached validation result remains valid.