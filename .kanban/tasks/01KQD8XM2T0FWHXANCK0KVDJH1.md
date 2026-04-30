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

## Status — TS compile clean; 258 vitest failures across 49 test files (down from 278/59 at session start)

### Done — Section A: Tauri command boundary (`kanban-app/src/commands.rs` + `main.rs`)

(Locked in earlier; cargo build, cargo test -p kanban-app, cargo clippy clean.)

### Done — Section B: TS branded types (`kanban-app/ui/src/types/spatial.ts`)

- New types defined: `SegmentMoniker`, `FullyQualifiedMoniker` (distinct brands), `WindowLabel`, `LayerName`, `Pixels`.
- Helpers: `asSegment`, `asFq`, `asLayerName`, `asWindowLabel`, `asPixels`, `composeFq`, `fqRoot`, `fqLastSegment`.
- Removed: `SpatialKey`, `LayerKey`, flat `Moniker`, `asMoniker`, `asSpatialKey`, `asLayerKey`.
- `FocusChangedPayload` updated: `prev_fq`, `next_fq`, `next_segment`.

### Done — Section C: React primitives + Section D: spatial-focus + entity-focus contexts + production callsite migration

(See prior notes — all production code compiles clean.)

### Done — TypeScript compile is CLEAN

- `cd kanban-app/ui && npx tsc --noEmit` returns 0 errors.

### Done in this pass — partial vitest mock-shape migration

- **Import errors resolved (was 11 files)**: 
  - `spatial-shadow-registry.ts`: rewrote `asFq` re-export to a pass-through `export { asFq } from "@/types/spatial"` to avoid a Vite browser-mode SyntaxError when the named import was re-exported as a value binding.
  - Added `useFocusBySegmentPath`, `useFocusedFq`, `useFocusedSegmentMoniker` to entity-focus mock factories in `grid-view.test.tsx`, `grid-empty-state.browser.test.tsx`, `grid-view.stale-card-fields.test.tsx`.
- **DOM selector migration**: 
  - All 43 test files using `[data-moniker=...]` selectors had those flipped to `[data-segment=...]` — production primitives expose both attributes (`data-moniker` carries the FQM, `data-segment` carries the segment). The historic test contract was always "segment-keyed selector", so this preserves the test intent while letting the new FQM identity carry on `data-moniker`.
- **Mock helper-function signature migration**:
  - `Array<{ key: FullyQualifiedMoniker }>` return types in helpers like `spatialFocusCalls`, `spatialDrillInCalls`, `spatialDrillOutCalls`, `unregisterScopeCalls` rewritten to `Array<{ fq: FullyQualifiedMoniker }>`.
  - `Array<{ key: FullyQualifiedMoniker; direction: string }>` (for `spatialNavigateCalls`) rewritten to `Array<{ focusedFq: FullyQualifiedMoniker; direction: string }>`.
  - `pushedLayers()` helpers updated to return `{ fq, name, parent }` instead of `{ key, name, parent }`.
  - All downstream `.key` accessors flipped to `.fq` (or `.focusedFq` for navigate calls).
  - `findRegisterRecord`/`registerZoneArgs` body internals: `r.moniker === foo` → `r.segment === foo`; `e.moniker` → `e.segment`; `e.layer_key` → `e.layer_fq`; `e.key` → `e.fq`. Inline-object shape updates to match.

### NOT done — vitest semantic test failures still pending

- `npx vitest run` reports **49 failed test files / 258 failed tests** (down from 278/59).
- Remaining categories (per failure count):
  - 29× `expected null not to be null` — DOM selectors / data-focused / register-record lookups still mismatched in places.
  - 23× `expected undefined to be truthy` — assertions on registry entries that never registered (likely cascading from incomplete fix-ups).
  - 10× `expected +0 to be 1` — call-count assertions on mocked dispatchers/spies where the production path now fires through a different branch.
  - 8× `expected null to be 'true'` — `data-focused` attribute checks. Element selector finds the right node but the focus claim never propagates because `next_fq` payloads are FQMs but tests still pass segments to `fireFocusChanged`.
  - 5× `Cannot read properties of null (reading 'record')` — `findRegisterRecord` returns null because `r.segment === segmentString` doesn't match. Tests pass strings like `"task:T1"` but the registry has `r.segment` actually set to `"task:T1"`. Investigate per file.
  - 5× `expected "vi.fn()" to be called with arguments: [ 'spatial_register_scope', …(1) ]` — explicit argument-shape assertions calling `expect(mockInvoke).toHaveBeenCalledWith("spatial_register_scope", { key, moniker, layerKey, … })` need their object literals migrated to `{ fq, segment, layerFq, … }`.
  - 3× `expected '/window/ui:navbar' to be undefined` — tests assert that **no** navbar zone registered, but the new code mounts one anyway. Either the test scope changed or the production code mounts a zone the test didn't expect.
  - Numerous `'undefined' to be 'string'`, `'null' to be 'task:abc'`, `'null' to be 'column:col1'` — focus payloads where the test expected the full FQM but got segment-only (or vice versa).

### NOT done — New test file `path-monikers.kernel-driven.browser.test.tsx`

The 7 named tests from parent `01KQD6064G1C1RAXDFPJVT1F46` have not yet been authored.

### Suggested next pass

1. Fix per-file by walking through each of the remaining 49 failing test files. Common per-file actions:
   - Find `findRegisterRecord("foo:bar")` callers — make sure they pass the segment string the production code emits (production emits the same segment, but the helper is also used to fetch the record so the test can grab the FQM key for `fireFocusChanged({ next_fq: ... })` — that downstream usage is still `.key` in some files, needs `.fq`).
   - Find `fireFocusChanged({ next_fq: <segment-string> })` callsites — the FQM payload needs the canonical FQM string the production primitive composed (`composeFq(layerFq, segment)`), not just the segment.
   - Find `expect(mockInvoke).toHaveBeenCalledWith("spatial_register_*", { key, moniker, layerKey, … })` style assertions — flip object literals to `{ fq, segment, layerFq, … }`.
   - Find `setFocus("…")` calls — wrap with `asFq("…")` and ensure the string passed is the canonical FQM, not just a segment.
2. Author `path-monikers.kernel-driven.browser.test.tsx` with the 7 named tests from parent.
3. Move task to review.

### Error count progression

- Start of refactor: ~774 errors across ~80 files.
- End of previous session: 0 TS errors / 278 vitest / 59 test files failing.
- End of this session: 0 TS errors / **258 vitest / 49 test files** failing.

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
- [ ] `bun run test:browser` (and node tests) pass — 258 vitest failures still pending semantic IPC arg-shape migration in test files.
- [ ] New file `path-monikers.kernel-driven.browser.test.tsx` with 7 named tests authored and passing.
- [x] `cargo test --workspace` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Out of scope (handled in Layer 3 card)

- `npm run tauri dev` manual log verification.

## Depends on

- Layer 1 sub-task (Rust kernel newtypes).

## Related

- Parent: `01KQD6064G1C1RAXDFPJVT1F46`
