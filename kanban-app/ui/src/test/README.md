# kanban-app UI tests

## Spatial-nav golden-path suite

`spatial-nav-golden-path.test.tsx` is the authoritative regression
suite for the spatial-navigation stack. It consolidates the basic
"nav works" invariants that define the user-visible contract:

- global invariants (dispatch routing, focus decoration)
- per-region nav (board, grid, inspector, left nav, perspective bar,
  toolbar)
- Enter activation (view switch, inspect, search, card-opens-inspector)
- cross-layer isolation (inspector over board/grid, three stacked
  inspectors, layer pop restores parent focus)
- visual focus invariants (exactly-one `data-focused` at all times,
  even under rapid-click bursts)

### Gate: any PR touching the spatial-nav stack must run this suite green

If a pull request modifies any of the following files, CI reviewers
must verify the golden-path suite runs green before merging:

- `kanban-app/ui/src/components/focus-scope.tsx`
- `kanban-app/ui/src/components/focus-layer.tsx`
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx`
- `kanban-app/ui/src/components/nav-bar.tsx`
- `kanban-app/ui/src/lib/entity-focus-context.tsx`
- `kanban-app/ui/src/lib/spatial-shim.ts`
- `swissarmyhammer-spatial-nav/src/spatial_state.rs`
- `swissarmyhammer-spatial-nav/src/spatial_nav.rs`

Run:

```sh
cd kanban-app/ui && npm test -- spatial-nav-golden-path
```

Every test name in the suite maps one-to-one to an invariant in the
task description that established the gate
(`01KPTT9X3HK5T7J5AMC6KHQHGQ`). A single failure is self-diagnostic:
read the test name, fix the invariant it names.

### Algorithm-level parity

The spatial-nav algorithm lives entirely in Rust
(`swissarmyhammer-spatial-nav`). Algorithm-level invariants — beam
testing, layer filtering, fallback-to-first, null-source recovery,
container-first sibling search — are exercised by the Rust scenario
table in `swissarmyhammer-spatial-nav/tests/spatial_cases.json`. There
is no JavaScript mirror of the algorithm, so there is no parity gap
to maintain; drift is impossible by construction.

Run the Rust scenarios with:

```sh
cargo test -p swissarmyhammer-spatial-nav
```

### What the golden-path suite does not cover

- Algorithm correctness ("which card wins a left beam test with
  three equally-near candidates?") — belongs in `spatial_cases.json`.
- UI state outside the spatial-nav contract (forms, drag-and-drop,
  command palette animation) — has its own dedicated tests.
- Visual styling beyond the `data-focused` attribute — covered by
  the component-level screenshot/snapshot tests.

### Coverage breakdown by invariant group

The suite pins React-side wiring for the spatial-nav contract. Some
invariants ride on Rust-side decisions (successor selection on
unregister, beam-test geometry, layer filtering) that the fixtures
cannot exercise without a live backend; those cases are pinned by
the parity tests in `swissarmyhammer-spatial-nav/tests/` and the
suite either models the backend's response via
`handles.scriptResponse(...)` / `handles.emitFocusChangedForMoniker(...)`
or stops short of asserting on the Rust-only portion.

- **Global invariants**: fully covered, including the "unmount
  transitions focus to a successor" invariant (backend response
  modelled via `emitFocusChangedForMoniker`).
- **Per-region nav**: one dispatch-half and one decoration-half test
  per edge transition listed in the task description.
- **Enter activation**: every scope type named in the task description
  has a test — LeftNav button, perspective tab, grid cell, inspector
  field, row selector, card (both keyboard and dblclick), toolbar
  inspect, toolbar search.
- **Cross-layer isolation**: layer push/pop counts, per-layer field
  registrations, and layer-pop focus restoration to the parent-layer
  scope (asserted via `data-focused` on the card after Escape).
- **Visual focus**: exactly-one `data-focused` across every click,
  rapid-click bursts, and nav round-trips.

A known gap — clicking a perspective tab `<div>` directly does not
focus the scope in the fixture due to a production-wiring detail; the
Enter-activation test works around it by seeding focus via a nav
keypress. See kanban task `01KPV65SPEX1RXHBHGSTPNQ5CJ` for the
investigation and fix plan.

### Adding a new invariant

1. Pick the smallest existing fixture that reproduces the scenario
   (`spatial-fixture-shell.tsx`, `spatial-grid-fixture.tsx`, etc.).
2. Add a named test to the right `describe` block in
   `spatial-nav-golden-path.test.tsx` whose name describes the
   invariant (e.g. `grid_k_from_top_body_row_reaches_column_header`).
3. Use scripted responses via `handles.scriptResponse(…)` to model
   what the Rust backend would do — do not re-implement the algorithm
   in JavaScript.
4. If the invariant is algorithmic (beam-test geometry, layer
   filtering) add a case to `spatial_cases.json` instead.
