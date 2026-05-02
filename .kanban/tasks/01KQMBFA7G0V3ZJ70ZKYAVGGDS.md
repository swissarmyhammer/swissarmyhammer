---
assignees: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffee80
title: 'RESOLVED: browser tests stale ui.setFocus contract — fixed inline during /test run'
---
RESOLVED — fixed inline during a /test run on branch `kanban`.

Three browser-mode test files asserted that drill-in / Enter / Escape / ArrowDown flows fan out to `dispatch_command("ui.setFocus", { args: { scope_chain: [...] } })` with the kernel-returned moniker at chain[0]. The assertions were stale relative to the production kernel-bridge architecture documented on `EntityFocusProvider`:

  - Production `FocusActions.setFocus(fq)` routes through `spatial.focus(fq)` → `spatial_focus` IPC, NOT through a `dispatch_command(ui.setFocus, ...)`.
  - The `subscribeFocusChanged` bridge inside `EntityFocusProvider` does dispatch `ui.setFocus` after each `focus-changed` event, but its `scope_chain` is built by looking up `payload.next_fq` (an FQM) in the entity-scope registry. The simulator's `spatial_focus` handler emits the raw moniker as `next_fq`, which doesn't match any FQM-keyed registry entry — so the chain was empty, never `[<moniker>, ...]`.

Fixes (production-aligned):
- kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx — replaced the `setFocusDispatch` lookup at lines 628-639 and 699-710 with a direct `spatial_focus` invocation assertion (`focusCall.fq === "task:t1"` / `"task:t2"`). This pins the same end-to-end contract — drill-in resolves through the kernel — without depending on bridge-derived scope chains.
- kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx — replaced the `setFocusDispatches()` helper with `spatialFocusCalls()` (and updated both call sites' assertions and rationale comments).
- kanban-app/ui/src/components/inspector.kernel-focus-advance.browser.test.tsx — the `ArrowDown from the last field stays put` test was inverted: real BeamNavStrategy at body→down does iter 0 (no peer) → iter 1 (no parent-zone peer) → drill-out fallback returns the parent panel `task:T1`. Renamed the test, rewrote the assertion to the correct drill-out behavior, and added a `waitFor(titleField.fq)` gate before the navigation loop so the test no longer races first-field auto-focus.

Also removed an unused `registerScopeArgs` helper from `kanban-app/ui/src/components/board-view.spatial.test.tsx` (TypeScript noUnused checks were failing under `tsc --noEmit`).

Verified clean:
- `cargo test --workspace` (now `cargo nextest run --workspace`) — 13576 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `npm test` (kanban-app/ui) — 1885 passed, 0 failed (4 pre-existing intentional skips remain — see follow-up tasks).