---
assignees:
- claude-code
depends_on:
- 01KQD8X3PYXQAJN593HR11T7R4
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffe380
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

## Review Findings (2026-04-30 07:07)

Verified the four user-stated invariants directly in code:

1. Consumer `moniker={...}` props are typed `SegmentMoniker` on `<FocusScope>`, `<FocusZone>`, `<Inspectable>`, and `<FocusLayer name=...>`.
2. `useFullyQualifiedMoniker()` exists in `kanban-app/ui/src/components/fully-qualified-moniker-context.tsx` and throws when called outside a primitive.
3. `setFocus(fq: FullyQualifiedMoniker | null)` and the Tauri commands (`spatial_focus`, `spatial_navigate`, `spatial_drill_in/out`, `spatial_push_layer`, `spatial_pop_layer`, `spatial_clear_focus`) accept only the FQM newtype; `path-monikers.kernel-driven.browser.test.tsx` test 6 pins the compile-error contract via `// @ts-expect-error`.
4. `FocusChangedPayload` carries `prev_fq` / `next_fq` / `next_segment`; `next_segment` is read-only display data, not an alternate identity.

Re-ran verification: `npx tsc --noEmit` 0 errors; `npx vitest run` reports `Test Files 179 passed (179) / Tests 1849 passed | 4 skipped (1853)`; `cargo test -p kanban-app` 93 passed; `cargo clippy -p kanban-app --all-targets -- -D warnings` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. (`cargo test --workspace` had a flaky failure in `shelltool-cli::commands::registry::tests::test_init_and_deinit_register_success_path` triggered by a malformed `.mcp.json` parse during another test's parallel run; passes when run in isolation, no shelltool-cli code touched on this branch.)

### Warnings
- [x] `ARCHITECTURE.md:27` — The Tier-0 spatial-focus-engine description still says the surface is "opaque `Moniker` strings, abstract `Rect`s, and `WindowLabel`s." That's now stale: after Layer 1 + Layer 2 the surface is `FullyQualifiedMoniker` + `SegmentMoniker` (distinct branded newtypes), not a single opaque `Moniker` type. The path-monikers identity model is the load-bearing change in this refactor — readers reaching for ARCHITECTURE.md to learn the focus-engine surface will be misled. Update the sentence to reflect the new newtype pair (and reference the `01KQD6064G1C1RAXDFPJVT1F46` rationale). **FIXED 2026-04-30**: rewrote the sentence to describe the `FullyQualifiedMoniker` + `SegmentMoniker` newtype pair and reference the `01KQD6064G1C1RAXDFPJVT1F46` rationale.

### Nits
- [x] `kanban-app/ui/src/components/inspectable.tsx:43-48,68` — Doc-comment usage examples still call `asMoniker(\`task:${task.id}\`)` and `asMoniker(entityMk)`. `asMoniker` was deleted in Section B and the example would not compile if it were real code. Update the examples to use `asSegment(...)` (since `<Inspectable moniker={...}>` takes a `SegmentMoniker`). **FIXED 2026-04-30**: replaced both `asMoniker(...)` calls with `asSegment(...)` in the two doc-comment examples.
- [x] `kanban-app/ui/src/components/board-view.tsx:1122` — Doc comment for `BoardSpatialZone` says `<FocusZone moniker={asMoniker("ui:board")}>`; replace `asMoniker` with `asSegment`. **FIXED 2026-04-30**.
- [x] `kanban-app/ui/src/components/app-shell.tsx:219, 272, 324` — Three doc-comment references describe the command closures as reading "the currently-focused [`SpatialKey`]" / "the currently-focused `(SpatialKey, Moniker)` pair". `SpatialKey` and the flat `Moniker` no longer exist; the closures read a `FullyQualifiedMoniker` via `actions.focusedFq()`. Update the prose to match. **FIXED 2026-04-30**: rewrote all three doc comments to describe `FullyQualifiedMoniker` read via `actions.focusedFq()`.
- [x] `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts:119` — Param doc says "the `SpatialKey` to push rect updates against." It is now a `FullyQualifiedMoniker`. (The same drift appears in a few other doc comments — `entity-card.tsx:270`, `focus-indicator.tsx:34` — but they are accurate when read as historical context; this one is on a current parameter and is misleading.) **FIXED 2026-04-30**: `key` param doc now says `FullyQualifiedMoniker`.

### Re-verification (post-fixes 2026-04-30)
- `cd kanban-app/ui && npx tsc --noEmit` → 0 errors (pure doc changes are runtime-no-op as expected).
