---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8d80
project: spatial-nav
title: 'Spatial nav: delete the JS algorithm mirror — Rust owns ALL computation, frontend is a dumb registrar'
---
## What

The frontend currently contains a second, parallel implementation of the entire spatial-navigation algorithm written in TypeScript. This is architecturally wrong: Rust owns the algorithm, the frontend must be a dumb registrar that:

1. Registers its scope rects via `invoke("spatial_register", ...)`
2. Dispatches commands (including `nav.*`) through the normal command pipeline
3. Subscribes to `focus-changed` events and updates a store
4. Derives visual state (`data-focused`) by comparing its own moniker to the store value

That's it. **No geometry, no beam test, no scoring, no layer filtering, no fallback-to-first, no parent_scope walk — none of it — in the frontend.**

### User's directive

> "why is this in the frontend at all — what the fuck were you doing — FocusScope needs to register in the backend — target boxes .. ALL AND I MEAN ALL computation and selection logic needs to be in the backend"
>
> "though that computation in the UI is fucked fucked fucked"

### The mirror that must be deleted

`kanban-app/ui/src/test/spatial-shim.ts` — a full TypeScript port of `swissarmyhammer-spatial-nav`. It reimplements:

- `majorAxisDistance` / `majorAxisDistanceToFarEdge` (beam-test math)
- `isInBeam` / `isInDirection` (cone filters)
- `score` (Android's `13 * major² + minor²`)
- `inBeamDominates` (arbitration rule)
- `findCardinal` (the pool-split / unified-ranking algorithm)
- `findTopLeft` (first-in-layer picker)
- `fallbackToFirst` (null/stale-source recovery)
- `spatialSearch` (layer-filtered candidate pool)
- `saveFocusMemory` / layer push-pop / last-focused restoration
- `applyOverride` (per-scope override map)

Plus a parity-fence infrastructure:

- `kanban-app/ui/src/test/spatial-parity-cases.json` — hundreds of shared test cases the two implementations must agree on
- `swissarmyhammer-spatial-nav/tests/parity.rs` — Rust half of the fence
- `kanban-app/ui/src/test/spatial-shim-parity.test.ts` — JS half of the fence

The only reason the shim exists is so vitest-browser tests can "exercise the full React click → setFocus → spatial_focus → focus-changed → data-focused loop without a Tauri backend." That's solving the wrong problem — it's duplicating the algorithm so React tests don't have to stub invoke(). The correct answer: React tests stub `invoke()` at the Tauri boundary and don't care what the algorithm computes. Algorithm tests live in Rust, where the algorithm lives.

### Consequences of the mirror

- **Duplicated logic drifts.** Every time Rust changes the algorithm (and the recent uncommitted diff made significant changes to `find_cardinal`, `in_beam_dominates`, and the `focus_first_in_layer` guard), the JS shim must be updated in lockstep. The parity fence catches drift but creates a coordination tax on every change.
- **Tests lie.** 1397 tests pass while the real app's nav is dead. That's because most tests run against the shim, which can be internally consistent while Rust is actually broken (or vice versa).
- **Frontend bloat.** ~700 LOC in `spatial-shim.ts` plus fixture code that wires it up. None of it ships to users. All of it gets reviewed, maintained, updated.

### What must change

1. **Delete** `kanban-app/ui/src/test/spatial-shim.ts` entirely.
2. **Delete** `kanban-app/ui/src/test/spatial-shim-parity.test.ts`.
3. **Delete** `kanban-app/ui/src/test/spatial-parity-cases.json` (the Rust side `swissarmyhammer-spatial-nav/tests/parity.rs` can either be kept as a JSON-driven case table inside Rust alone, or unified with `spatial_state.rs::tests` — up to the implementer).
4. **Delete** the shim wiring from `kanban-app/ui/src/test/setup-spatial-shim.ts` (the `vi.mock` factories that route `@tauri-apps/api/core`/`event`/`webviewWindow` invoke calls into the shim). Replace with minimal stubs that:
   - Intercept `invoke("spatial_register", ...)` / `invoke("spatial_unregister", ...)` → no-op (frontend rect registration doesn't need a backend in React tests)
   - Intercept `invoke("spatial_navigate", ...)` / `invoke("spatial_focus", ...)` → the test controls what `focus-changed` event the stub emits, without reimplementing the algorithm
   - Intercept `listen("focus-changed", ...)` → return an emitter the test drives manually
5. **Rewrite fixtures** (`spatial-fixture-shell.tsx`, `spatial-grid-fixture.tsx`, `spatial-board-fixture.tsx`, `spatial-inspector-fixture.tsx`, `spatial-leftnav-fixture.tsx`, `spatial-perspective-fixture.tsx`, `spatial-toolbar-fixture.tsx`, `spatial-multi-inspector-fixture.tsx`, `spatial-inspector-over-grid-fixture.tsx`) to use the stubbed Tauri boundary. They should set up React state, dispatch commands, and assert on React state + DOM — **not** manually compute "which scope should be focused next."
6. **Refactor nav tests** (`spatial-nav-board.test.tsx`, `spatial-nav-canonical.test.tsx`, `spatial-nav-grid.test.tsx`, `spatial-nav-inspector.test.tsx`, `spatial-nav-leftnav.test.tsx`, `spatial-nav-perspective.test.tsx`, `spatial-nav-toolbar.test.tsx`, `spatial-nav-multi-inspector.test.tsx`, `spatial-nav-inspector-over-grid.test.tsx`) — each test should either:
   - Assert "pressing `j` dispatches `nav.down` to the backend" (React-level), stubbing the `focus-changed` event the backend would emit, OR
   - Be MOVED to Rust if it's really an algorithm test masquerading as a React test
7. **Algorithm coverage moves to Rust.** Every case in `spatial-parity-cases.json` becomes a Rust unit test in `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests` or an integration test in `swissarmyhammer-spatial-nav/tests/`. Rust is the sole place the algorithm is tested.

### What stays in the frontend

- `FocusScope` — still registers via `invoke("spatial_register", ...)`; subscribes to `useFocusedMoniker()`; imperatively sets `data-focused`. No changes here.
- `FocusLayer` — still pushes/pops layers via Rust invokes. No changes.
- `entity-focus-context.tsx` — keeps `spatial_key ↔ moniker` maps for the invoke arg translation, keeps the `focus-changed` listener, keeps the focused-moniker store. Does NOT keep `NAV_DIRECTION_MAP`, `useBroadcastNav`, `broadcastNavCommand` — those are deleted per `01KPTV64JF4QJE6GQK3DS0TK41`.

### Files to modify / delete

Delete:
- `kanban-app/ui/src/test/spatial-shim.ts`
- `kanban-app/ui/src/test/spatial-shim-parity.test.ts`
- `kanban-app/ui/src/test/spatial-parity-cases.json`

Rewrite:
- `kanban-app/ui/src/test/setup-spatial-shim.ts` — minimal Tauri-boundary stub only; rename to `setup-tauri-stub.ts` since "shim" is being retired as a concept
- Every `spatial-*-fixture.tsx` — remove shim imports, use stubs
- Every `spatial-nav-*.test.tsx` — assert at the React/dispatch boundary, not against a JS-computed outcome

Keep and likely expand:
- `swissarmyhammer-spatial-nav/src/spatial_state.rs::tests` — absorb all algorithm coverage from the deleted parity JSON
- `swissarmyhammer-spatial-nav/tests/` — integration tests around layer push/pop, fallback, override; can be reorganised from the deleted `parity.rs`

### Relationship to other tasks

- `01KPTV64JF4QJE6GQK3DS0TK41` (rip out broadcastNavCommand) — complementary. That task deletes the JS-side NAV command side-channel. This task deletes the JS-side ALGORITHM mirror. Together they enforce "Rust owns it all."
- `01KPTVARMZR21964VRVJMP32GF` (focus_first_in_layer guard) — unrelated to the JS mirror, but blocks boot-time focus. Fix that first; then this task becomes a pure cleanup against a correctly-working Rust algorithm.
- `01KPTT9X3HK5T7J5AMC6KHQHGQ` (golden-path regression suite) — the golden-path tests written for THAT task must be written against the REAL dispatch path, not the shim. If this task lands first, the golden path has less to fight against.
- `01KPS1WCQRY8DEWQVA47PZ82ZC` through `01KPTJMZCD758KFXRHJN7ZA52H` — most tests added by those tasks are written against the shim. They must be rewritten or deleted as part of this task's rollout.

### Out of scope

- Changing what the algorithm does (the algorithm itself is Rust; this task is about where the algorithm lives)
- Changing the wire format between JS and Rust
- Adding new Tauri commands beyond what's needed to retire the shim

## Acceptance Criteria

- [ ] `kanban-app/ui/src/test/spatial-shim.ts` does not exist in the repo
- [ ] `kanban-app/ui/src/test/spatial-shim-parity.test.ts` does not exist in the repo
- [ ] `kanban-app/ui/src/test/spatial-parity-cases.json` does not exist in the repo
- [ ] No file under `kanban-app/ui/` contains `majorAxisDistance`, `isInBeam`, `findCardinal`, `findTopLeft`, `fallbackToFirst`, or any other algorithm helper from the deleted shim — grep returns zero hits
- [ ] Every `spatial-nav-*.test.tsx` file still exists and asserts at the React/dispatch boundary, not against JS-computed geometry
- [ ] `cargo test -p swissarmyhammer-spatial-nav` has at least as much algorithm coverage as the deleted parity JSON (each parity case absorbed into a Rust test or deliberately dropped with rationale)
- [ ] `cd kanban-app/ui && npm test` passes
- [ ] The diff for this task shows net negative LOC in `kanban-app/ui/src/test/` (we're deleting duplicated work)

## Tests

- The tests for this task are the deletion itself plus the refactored React-boundary tests.
- [ ] `cd kanban-app/ui && npm test` — green, no shim, no parity
- [ ] `cargo test -p swissarmyhammer-spatial-nav` — green with increased coverage
- [ ] Manual: launch the app, navigate with h/j/k/l, verify focus moves. If this task is done correctly and the algorithm is correct in Rust, nav works — because nav was always supposed to be Rust's job.

## Workflow

- Sequence: finish `01KPTVARMZR21964VRVJMP32GF` (boot focus) and `01KPTV64JF4QJE6GQK3DS0TK41` (nav command dispatch) FIRST so the frontend actually works end-to-end with Rust. Then rip out the shim.
- Work in the following order to minimize breakage:
  1. Absorb any parity case not already covered in Rust into a Rust unit test
  2. Rewrite one fixture + its tests against the Tauri-boundary stub; confirm green
  3. Repeat per fixture
  4. Delete the shim files when no fixture references them
  5. `grep -r majorAxisDistance kanban-app/ui` → must return zero
- Resist the urge to "keep the shim for a bit just in case." It's dead code the moment Rust is authoritative. Delete in the same commit that proves the Rust path works end-to-end.

