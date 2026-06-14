---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8d80
project: ui-command-cleanup
title: 'Harden per-window op resolution: no silent default-to-main when the window: moniker is missing'
---
## What
The redundant `window_label` op parameter is ALREADY removed and per-window `ui_state` ops resolve the window from the scope chain's `window:<label>` moniker (commit `a2002c330`, 2026-06-05). This card is the REMAINING gap, not a duplicate.

`window_from_scope` in `crates/swissarmyhammer-ui-state/src/service.rs` ended in a **silent fallback** (`unwrap_or(DEFAULT_WINDOW_LABEL)` → "main"). If any caller sent a scope chain lacking a `window:` moniker, every per-window op silently wrote to the `main` slot — the exact regression class `a2002c330` fixed.

## Goal
Make a missing `window:` moniker a loud, detectable failure on the per-window op path instead of a silent default-to-main, and add a systemic guarantee that per-window ops always carry the window.

## Implementation (done)
- `window_from_scope` now returns `Result<&str, McpError>`: on a chain with no `window:` moniker it emits a `tracing::error!` (observable in the unified log) and returns an `invalid_params` error naming the op and the offending chain. `DEFAULT_WINDOW_LABEL` is deleted — no default remains anywhere in the service.
- All 11 per-window mutation handlers propagate the error: inspect/close/close_all/set_width inspector, open/close palette, set active_view, show command/palette/search, dismiss ui. (`start rename` is a backend no-op that resolves no window; keymap/scope_chain/drag ops are global by design.)
- Producer side verified: every dispatch site sits under `WindowContainer`'s `CommandScopeProvider moniker="window:{WINDOW_LABEL}"`; no legitimate windowless caller of the per-window set exists.
- Residual app-side `"main"` fallbacks OUTSIDE the ui_state op path (menu.rs `open_and_notify`, dormant `BoardSwitch`/`BoardClose` handlers, dead legacy kanban-crate command impls) are inventoried in follow-up card `01KTSGSZ6672S9H1S0TZTRFPCA`.

## Acceptance Criteria
- [x] A per-window op invoked with a scope chain lacking a `window:` moniker returns an error (or is otherwise rejected) — it does NOT mutate the `main` window.
- [x] The silent `unwrap_or("main")` default is gone from the per-window mutation path (read-only/global helpers may keep an explicit, documented default if justified). — `DEFAULT_WINDOW_LABEL` deleted entirely; no default remains.
- [x] The frontend-produced scope chain for per-window dispatches always contains a `window:<label>` moniker (producer-side guarantee), verified by a test. — Pre-existing tests cover this: `command-scope.test.tsx` (dispatched `scope_chain` ends in `window:main` / `window:board-2`), `use-dispatch-command.test.tsx`, `entity-focus-context.test.tsx` ("scope chain built from a focused entity includes window:main at the root"). Re-verified: 85/85 pass.

## Tests
- [x] `swissarmyhammer-ui-state` service test: drive each per-window op with a scope chain that has NO `window:` moniker → assert it errors and that neither the `main` slot nor any window slot was mutated. — `per_window_ops_reject_scope_chain_without_window_moniker` (all 11 ops; asserts `all_windows()` stays empty) + `per_window_op_rejects_empty_scope_chain`.
- [x] Positive test (already exists per a2002c330 — keep/extend): a non-"main" `window:` moniker flips the correct window's state and nothing lands on `main`. — added `per_window_op_targets_the_scope_chain_window_not_main` (`set active_view` on `window:board-2` leaves `main` untouched).
- [x] Frontend/producer test: the scope chain sent for a per-window command includes the `window:` moniker. — pre-existing, see above.
- [x] Regression: fails before the hardening (silent main write), passes after. — RED observed: both rejection tests failed with `got: {"ok":true,...}` (silent main write) before the resolver change; GREEN after: ui-state 136/136.

## Verification
- `cargo nextest -p swissarmyhammer-ui-state`: 136/136
- `cargo nextest -p swissarmyhammer-command-service` (drives the builtin TS plugins through the real `UiStateServer`): 125/125 (baseline)
- `cargo nextest -p swissarmyhammer-focus`: 121/121 (baseline)
- scoped vitest (producer tests): 85/85

## Related
- Builds on `a2002c330` (scope-chain window resolution).
- Blocks `01KTCRX1AP2WHKM4BPHWG7XYJT` (view.set has no effect): `view.set` routes to ui_state `set active_view`, a per-window op. If its dispatch chain lacks `window:`, the active view was previously recorded on `main` with zero feedback — the exact reported symptom. After this hardening that failure mode is a loud dispatch error naming the op + chain in the unified log, so the residual (if it reproduces) is now directly diagnosable.
- Follow-up: `01KTSGSZ6672S9H1S0TZTRFPCA` (app-side "main" fallbacks outside the ui_state op path). #tech-debt