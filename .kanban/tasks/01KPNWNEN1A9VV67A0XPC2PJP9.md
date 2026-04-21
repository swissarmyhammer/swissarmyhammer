---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff280
project: spatial-nav
title: 'Board: h/j/k/l nav between cards across columns (regression-proof with tests)'
---
## What

In the board view, h/j/k/l should move focus between cards:
- `j` / `k` moves within a column (to the card below / above the focused card)
- `h` / `l` moves between columns (to the nearest card in the column to the left / right)
- At a column or row edge, nav clamps — doesn't wrap around

Manual testing confirms this mostly works today ("nav works on board and inspector" once the camelCase serde fix landed). But there is ZERO automated test for it — a regression is one bad edit away, as repeatedly demonstrated this session.

### Harness choice

The task originally specified the `kanban-app/e2e/` WebdriverIO + tauri-driver harness (`--only` hermetic launch), but that harness was removed in commit fce43edc9 ("refactor(kanban-app): remove --only hermetic launch mode"). Parallel tasks have since built out the vitest-browser `SpatialStateShim` harness (`kanban-app/ui/src/test/spatial-shim.ts`, `setup-spatial-shim.ts`, `spatial-grid-fixture.tsx`) which exercises the same React code paths against a JS shim that is behavior-equivalent to the Rust spatial engine (verified by `spatial-shim-parity.test.ts`). These tests use that harness, mirroring the grid and inspector fixtures.

### Tests

`kanban-app/ui/src/test/spatial-nav-board.test.tsx` codifies the four contract tests from the task description:

- `j` moves focus from card(col 1, row 1) to card(col 1, row 2)
- `l` moves focus across columns at the same vertical level (card-1-2 → card-2-2)
- `j` at the bottom card of a column stays put (clamped)
- `h` at the leftmost column does NOT wrap to the rightmost column

Built on top of `kanban-app/ui/src/test/spatial-board-fixture.tsx` (3 columns × 3 cards) — analogous to `spatial-grid-fixture.tsx`.

### Subtasks

- [x] Create `kanban-app/ui/src/test/spatial-board-fixture.tsx` — 3 columns × 3 cards board fixture using real `FocusScope` / `FocusLayer` / `EntityFocusProvider`. Column and card rects share horizontal edges so the spatial engine's `13*major² + minor²` scoring picks the adjacent card over the adjacent column (the tiny padding delta that would otherwise tip the score in the wrong direction is avoided). The fixture also wires `extractScopeBindings` into `createKeyHandler` — this is the key detail that makes `j`/`k`/`h`/`l` actually resolve against the scope-chain commands (production binds nav keys per-scope via the focused-scope binding table, not in the global vim table).
- [x] Write the 4 contract tests above in `kanban-app/ui/src/test/spatial-nav-board.test.tsx`
- [x] Run tests — 4/4 pass on 3 consecutive green runs (confirming "codified existing behavior")
- [x] No regression in grid / inspector nav — the full `src/test/` suite runs 25 tests green (5 test files: parity, canonical, inspector, grid, board); `focus-scope` and `column-view` tests (63 total) also pass unchanged.

### Acceptance

- [x] All 4 tests pass reliably (3 consecutive green runs)
- [x] `spatial-nav-board.test.tsx` runs as part of the default vitest browser suite (`npx vitest run --project browser` picks it up automatically via the `src/**/*.test.{ts,tsx}` glob).

## Review Findings (2026-04-20 14:35)

### Warnings
- [x] `kanban-app/ui/src/test/spatial-board-fixture.tsx:110-183` — `FixtureKeybindingHandler`, `useFixtureNavCommands`, and `FixtureShell` are now duplicated verbatim across three fixtures (`spatial-grid-fixture.tsx:110-177`, `spatial-inspector-fixture.tsx`, and this file). The pattern predated this task, but adding a third copy makes the duplication worse. Extract `FixtureShell` + `FixtureKeybindingHandler` + `useFixtureNavCommands` into a shared `kanban-app/ui/src/test/spatial-fixture-shell.tsx` so all three fixtures import a single source of truth — if production's `AppShell` keybinding wiring changes, only one test helper needs updating, otherwise the fixtures silently drift.

Resolved: Extracted `FixtureShell`, `FixtureKeybindingHandler`, and `useFixtureNavCommands` into `kanban-app/ui/src/test/spatial-fixture-shell.tsx`. All three fixtures now import from the shared module. The shared shell takes optional `extraCommands` (inspector uses this for its `ui.inspect` handler) and `navOverrides` (inspector uses `{ navFirstVim: "g g", navLastVim: "Shift+G" }`). `AppWithInspectorFixture` now renders `<InspectorBody>` as a sibling of the fixture body inside the shared shell's `CommandScopeProvider`. All 1301 tests still pass.

### Nits
- [x] `kanban-app/ui/src/test/spatial-nav-board.test.tsx:136,153` — Arbitrary `await new Promise((r) => setTimeout(r, 100))` after the clamp/no-wrap keystrokes is a test-smell: if spatial nav gets slower than 100ms in the future, these negative assertions will false-pass (report "no change" while a navigation is still in flight). Prefer either `await vi.waitFor(...)` polling for a concrete "settled" condition, or document why a fixed wait is safer here (e.g., "the shim is synchronous; this only accommodates React flush"). A short comment next to the wait citing the sync-shim guarantee would be enough.

Resolved: Replaced both comments with explicit sync-shim rationale — the `SpatialStateShim` is strictly synchronous (parity is locked down by `spatial-shim-parity.test.ts`), so `invoke("spatial_navigate", ...)` resolves before the wait runs; the 100ms buffer only covers React's commit loop, not spatial nav latency, so the test stays valid even if the engine gets slower. Both tests still pass.

- [x] `kanban-app/ui/src/test/spatial-board-fixture.tsx:256` — Column width `"111px"` is unexplained. If this value was empirically tuned to satisfy the `13*major² + minor²` scoring contract (plausible given the fixture's comment on edge-alignment), promote it to a named constant with a one-line comment — e.g. `const COLUMN_WIDTH_PX = 111; // tuned so card rects dominate column rects in the spatial score (see FixtureColumn note)`. As-is, a future maintainer changing it to `100px` or `120px` won't know whether the tests are asserting against an implementation detail or a real contract.

Resolved: Promoted the magic number to a top-level `COLUMN_WIDTH_PX = 111` constant with a docblock explaining that the value is tuned for the beam-test scoring contract (see `FixtureColumn` for the rationale). `FixtureColumn` now interpolates the constant into its inline `width` style.