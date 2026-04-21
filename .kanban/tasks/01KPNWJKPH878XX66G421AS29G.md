---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffff780
project: spatial-nav
title: Investigate "jumps to first cell and sticks" intermittent in grid nav
---
## What

During manual testing, once the camelCase serde fix landed and cell FocusScopes were added, grid nav mostly worked — but the user reported an intermittent bug: "sometimes it'll jump to the very first cell and stick there". Further nav keys from that state do nothing.

This is consistent with Rust `SpatialState::navigate` falling back to the `First` direction when the source key isn't registered in `entries`. The React side sends whatever key is first in `monikerToKeysRef.get(moniker)`; if that key has been unregistered on the Rust side (scroll-off, unmount, StrictMode double-mount race, stale cell key left in the map), Rust returns First.

### Harness / infrastructure already available (2026-04-20 19:45)

From `01KPNWGFTF` + `01KPNWH82X`:
- `kanban-app/ui/src/test/spatial-shim.ts` — the JS shim implements the same `fallbackToFirst()` path. Test scenarios can force a missing-source-key condition.
- Grid, board, inspector fixtures — pick one based on the repro scenario.
- `__spatial_dump` works client-side via the shim.

The shim's `fallbackToFirst` is a faithful port of Rust's behavior (parity test confirms). Any repro written against the shim will also fail against Rust, and vice versa.

### Resolution (2026-04-20)

The bug was reproduced reliably in the vitest-browser harness by forcing a React/Rust desync: after clicking a cell and silently dropping Rust's entry for the focused key (bypassing the focus-changed event), pressing `j` caused the exact "jump to first cell and stick" symptom — Rust's `fallback_to_first` picked the top-left entry, whose key frequently had no mapping in React's `keyToMonikerRef`, so React's focused moniker was cleared and subsequent nav keys became no-ops.

**Fix**: removed the fallback-to-First behavior. `SpatialState::navigate` (Rust) and `SpatialStateShim.navigate` (JS) now return `Ok(None)` / `null` when the source key is unknown. The frontend treats unknown-source navigates as no-ops; focus stays put visually, and the next `focus-changed` event or a user click reconciles state.

Files touched:
- `swissarmyhammer-spatial-nav/src/spatial_state.rs` — removed `fallback_to_first` helper; `navigate` returns `Ok(None)` on unknown source; unit tests updated accordingly.
- `kanban-app/ui/src/test/spatial-shim.ts` — removed `fallbackToFirst`; `navigate` returns `null` on unknown source.
- `kanban-app/ui/src/test/spatial-parity-cases.json` — updated the "unknown-key navigate" parity case to assert no-op behavior.
- `kanban-app/ui/src/test/spatial-nav-stale-key-repro.test.tsx` — new regression test that would have failed pre-fix.

### Acceptance

- [x] Confirmed bug still reproduces (reliably reproduced via the shim-based desync test)
- [x] A reliable failing test that reproduces the jump-to-first-and-stick behavior in the vitest-browser harness (`spatial-nav-stale-key-repro.test.tsx`)
- [x] Root cause documented (Rust's `fallback_to_first` hides React/Rust desync and silently yanks focus to a key React often can't map back to a moniker)
- [x] Fix that makes the test pass without breaking any existing grid / board / inspector nav tests (all 41 spatial-nav tests pass; full UI suite 1318 tests pass; Rust spatial-nav + kanban-app suites 138 tests pass; parity test passes)
- [x] No regression in the canonical `j` test

### Depends on

- `01KPNWH82X` (grid cells, in `doing`) — the grid fix may have eliminated the bug; re-check once that lands