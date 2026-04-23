---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffef80
project: spatial-nav
title: 'Test harness: vitest-browser integration for spatial nav (React → Tauri → Rust → DOM)'
---
## What

Every spatial nav "fix" this session was done blind — no failing test, no reproducible scenario, just live code edits watched against a running dev app. That is exactly why grid nav regressed every second change. We need a TDD harness sufficient to drive a real React tree, invoke the spatial commands against a deterministic Rust state machine, and assert against the DOM — **using vitest only, no WebDriver / tauri-driver / external process.**

### Why NOT tauri-driver / WebdriverIO

The previous webdriver-based E2E attempt (`kanban-app/e2e/spatial-nav.e2e.ts`, `wdio.conf.ts`, tauri-driver install requirement) failed to run reliably and was removed in commit `89e08625c`. Do not resurrect it. CI and local dev must stay on vitest.

### Approach — vitest-browser with a deterministic invoke mock

Two complementary tiers.

**Tier 1. Rust state-machine tests** (already exist in `swissarmyhammer-spatial-nav/src/spatial_state.rs` + `swissarmyhammer-spatial-nav/src/spatial_nav.rs`): pure unit tests of the beam-test algorithm, layer stack, focus memory, overrides. These stay as-is and catch regressions in the core algorithm.

**Tier 2. React vitest-browser tests with a `SpatialState` JS shim:** a small TS implementation of the navigation + registry routines that the Rust command handlers would otherwise drive. Route `@tauri-apps/api/core`'s `invoke` through `vi.mock(...)` to the shim. Tests render the full React tree (`AppShell`, `FocusLayer`, `FocusScope`s, the board/grid/inspector components) against this shim so:

- `spatial_register` → shim's registry Map
- `spatial_focus` → shim's focused_key + emit `focus-changed` synchronously
- `spatial_navigate` → shim runs the beam test + scoring and emits `focus-changed`
- `spatial_push_layer` / `spatial_remove_layer` → shim's layer stack
- `__spatial_dump` → shim returns its internal state

The shim MUST be behavior-equivalent to Rust — verified by shared test fixtures (a list of `(before-state, command, after-state)` triples run against both implementations, or a parity test that imports cases from a JSON file built from Rust tests). Specifying identical behavior in two implementations is the trade-off; the payoff is vitest-only CI and fast local iteration.

**What the Tier 2 tests cover that Tier 1 doesn't:**
- DOM rect measurement via a real `ResizeObserver` polyfill (happy-dom / jsdom) producing numeric rects from the rendered tree
- Click handlers actually running through React's event system
- `focus-changed` event listener wiring, claim callbacks flipping `data-focused`
- Scope chain walking from the focused FocusScope up through CommandScopeProviders
- Keybinding resolution picking the right `execute` handler

### Failing test (the first one to write)

```tsx
it("grid: pressing 'j' from cell (0,0) moves focus to cell (1,0)", async () => {
  const { shim } = setupSpatialShim();
  render(<AppWithGridFixture />);   // renders 3x3 tags grid fixture
  const cell00 = screen.getByTestId("data-moniker:field:tag:tag-0.tag_name");
  await userEvent.click(cell00);
  await waitFor(() => expect(cell00).toHaveAttribute("data-focused", "true"));
  await userEvent.keyboard("j");
  const cell10 = screen.getByTestId("data-moniker:field:tag:tag-1.tag_name");
  await waitFor(() => expect(cell10).toHaveAttribute("data-focused", "true"));
});
```

MUST fail against HEAD today because cells aren't FocusScopes yet.

### Subtasks

- [x] Implement the JS `SpatialState` shim in `kanban-app/ui/src/test/spatial-shim.ts` — navigate, beam test, scoring, layers, focus memory, overrides
- [x] Parity tests: run a shared case list against both Rust and the JS shim, diff the outputs, fail if they diverge (prevents drift)
- [x] `setupSpatialShim()` test helper that mocks `invoke` and wires `focus-changed` emission synchronously
- [x] A fixture factory that renders the full app tree against a deterministic board/tag grid / inspector setup (no real Tauri backend, just React + the shim)
- [x] Write the canonical failing `j` test above
- [x] Extend with sibling-task scenarios as they land (grid cells, row selector, LeftNav, perspective bar, inspector trap)

### Acceptance

- [x] `npm test` (vitest) runs these tests
- [x] No WebDriver / tauri-driver dependency
- [x] JS shim ↔ Rust parity test green (both implementations agree on all fixture scenarios)
- [x] At least the canonical `j` test is present and failing against today's HEAD
- [x] Sibling nav tasks can reliably add their own vitest scenarios without re-inventing infrastructure

### Scope explicitly excluded

- Any form of real Tauri IPC — not in this task, not in CI, not locally
- Any spawning of external processes
- Playwright, WebdriverIO, tauri-driver, Cypress, etc.

### Implementation notes

- JS shim lives at `kanban-app/ui/src/test/spatial-shim.ts`, exporting `SpatialStateShim` + pure algorithm helpers (`findTarget`, `containerFirstSearch`).
- Parity fixtures in `kanban-app/ui/src/test/spatial-parity-cases.json` — consumed by both sides:
  - JS parity test: `kanban-app/ui/src/test/spatial-shim-parity.test.ts` (11 cases pass).
  - Rust parity test: `swissarmyhammer-spatial-nav/tests/parity.rs` (same 11 cases pass against production `SpatialState`).
- `setupSpatialShim()` + mock factories in `kanban-app/ui/src/test/setup-spatial-shim.ts`. Mock factories are exported so each test file invokes `vi.mock(...)` with them literally — works around vitest's hoist-per-file limitation.
- Grid fixture at `kanban-app/ui/src/test/spatial-grid-fixture.tsx` (3x3 tags grid with row-level FocusScopes, cells as plain divs — matches production DataTableRow).
- Canonical test at `kanban-app/ui/src/test/spatial-nav-canonical.test.tsx`, marked `it.fails` so vitest reports it as "expected fail" today and "unexpectedly passed" once cells become FocusScopes.