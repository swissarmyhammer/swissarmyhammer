---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff880
title: Fix drill-in / drill-out (Escape) regression from window-from-fq-root change
---
## What

Commit `efbdf3783` ("fix(focus): derive owning window from the fq root segment") fixed jump (clears prior focus, multi-window isolated) by making every focus op derive the owning window from the window-rooted fq root segment (path wins over the explicit `window` arg and the layer side field). That change regressed **drill-in** and **drill-out (Escape)**: drilling no longer commits/moves focus correctly.

## Root cause (the REAL navigate-vs-drill asymmetry, ABOVE the kernel)

The kernel is SYMMETRIC for navigate and drill (both `reconcile_slot` + `focus`, both derive window from `fq.root_segment()`), so the kernel cannot explain why the user sees navigate WORK but drill/Escape BREAK live. The divergence lives in the `nav-commands` plugin's drill SOURCE-resolution:

- `nav.up/down/left/right` (navigate) send only `{ window, direction }` and rely on the server `resolve_nav_source` fallback chain: wire `focused_fq` → `provider.focus(window)` → **kernel `focus_by_window` slot fallback**. So navigate still moves when the UI provider returns nothing but the kernel slot is set.
- `nav.drillIn`/`nav.drillOut` (pre-fix) FIRST called `this.focusedFq(focus, window)` → focus `query focus` → `handle_query_focus` → `provider.focus(&window)` ONLY — **no kernel-slot fallback**. When `provider.focus(window)` returns undefined (the live UI-provider gap), drillIn silently no-op'd (`{ ok:true, next_fq:null }`) and drillOut fell through to `dismiss` instead of drilling. That is exactly the live "navigate works / drill broke" signature.
- The server `resolve_drill_source` ALREADY had the same provider→kernel-slot fallback as `resolve_nav_source`, but the plugin bailed before ever reaching it (and inline `focused_fq` on the wire short-circuits it in the old tests).

## Fix (production path — plugin + server src)

1. **Plugin** (`builtin/plugins/nav-commands/index.ts`): `nav.drillIn`/`nav.drillOut` no longer pre-resolve focus via `query focus`. They send ONLY `{ window }` and let the server `resolve_drill_source` resolve the source through the SAME provider→kernel-slot fallback navigate uses. Removed the dead `focusedFq()` helper and the `query` dispatch surface. drillOut's dismiss fall-through now keys on a server-provided `moved` flag instead of the old client-side `next_fq === focusedFq` echo check (which required pre-resolving focus — the source of the bug).
2. **Server** (`crates/swissarmyhammer-focus/src/server.rs`): `handle_drill_in`/`handle_drill_out` use the resolved focus as the drill `fq` when the wire omits it (`req.fq.unwrap_or(focused_fq)`), and now return a `moved` boolean (true iff focus actually changed) so the plugin can decide the dismiss fall-through without pre-resolving focus.
3. **Operations** (`crates/swissarmyhammer-focus/src/operations.rs`): `DrillIn.fq` / `DrillOut.fq` made `Option` (`#[serde(default)]`) so the host-driven plugin path can omit them; the React inline path (`focus-mcp.ts`) is unchanged (still sends `fq`, deserializes fine).

## Acceptance Criteria
- [x] Drill-in commits focus to the child target and emits a FocusChangedEvent for the correct window
- [x] Drill-out (Escape) commits focus to the parent target and emits a FocusChangedEvent for the correct window
- [x] Jump behavior from efbdf3783 stays correct (prior focus cleared, multi-window isolated) — no regression
- [x] Two windows on the same board: drill in one window does not move focus in the other
- [x] Drill resolves its current-focus SOURCE through the same kernel-slot fallback navigate uses (no longer provider-only) — the live navigate-works/drill-broke asymmetry is fixed in the production path

## Tests
- [x] Kept the prior kernel-property guards in `two_window_isolation.rs` (`drill_in_then_out_round_trip_*`, `drill_in_window_a_leaves_window_b_focus_untouched`) — valid, but they are NOT the live-regression guard (they put `focused_fq` inline, short-circuiting source resolution)
- [x] Added the REAL live-regression guard: `builtin_nav_commands_e2e::drill_resolves_source_via_kernel_slot_when_ui_focus_is_absent`. Drives `nav.down` then `nav.drillIn` HOST-DRIVEN (no inline `focused_fq`) through the real plugin, with a window-sensitive `GapProvider` whose `focus()` returns `None` (UI gap) while the kernel slot IS seeded. Navigate moves (control); drill must reach the server drill op and resolve its source from the kernel slot.
- [x] Red-green-red verified: pre-fix the test fails DRILL-SPECIFICALLY (navigate assertions pass, drill returns `next_fq: null` — the plugin bailed via `query focus`); after the plugin+server fix it passes; re-introducing only the plugin pre-resolution (server fix still in place) makes it RED again — proving the plugin change is load-bearing
- [x] `cargo nextest run -p swissarmyhammer-focus` — 120 tests, all green
- [x] `cargo nextest run -p swissarmyhammer-command-service builtin_nav_commands` — 2 tests, all green (the new regression guard + the existing nav e2e)

## Out of scope (pre-existing, unrelated failures noted)
`builtin_ui_commands_e2e` and `builtin_app_shell_commands_e2e` fail on the `plugin` branch on keybinding-metadata assertions (`ui.inspector.close keys`, `app.dismiss keys`) — unrelated to focus/drill; those bindings were changed by card `01KTPDTH772HSEV5F7R1DKYDNJ`. Not touched here. The 2 `clippy::too_many_arguments` warnings on `SpatialState::focus_lost` (state.rs) are also pre-existing and untouched.

## Workflow
- Use `/tdd` — write the failing drill regression test first, then fix to make it pass.

## Review Findings (2026-06-09 15:25)

Verdict: BLOCKER — stays in `review`. The change is test-only (no `src/` edits). The new tests are well-built and the focus-crate suite is genuinely green (120/120, re-verified). RED-GREEN was also re-verified: the two new drill tests fail against pre-efbdf3783 kernel semantics and pass against current. BUT the central claim — that the regression is reproduced-and-fixed — does not hold. The tests prove a *symmetric kernel property*; they neither reproduce nor explain the user's LIVE *asymmetric* signature (jump/navigate WORK, drill BROKE), and the actual production drill source-resolution path that can diverge is left untested.

### Blockers
- [x] `crates/swissarmyhammer-focus/src/state.rs` + `crates/swissarmyhammer-focus/src/server.rs` — The kernel/server treat navigate and drill SYMMETRICALLY. RESOLVED: confirmed the kernel is symmetric and cannot explain the live divergence; the fix landed ABOVE the kernel in the plugin (and the server `fq`/`moved` seam), not in the kernel. Kernel state.rs untouched.

- [x] `builtin/plugins/nav-commands/index.ts` — The REAL navigate-vs-drill asymmetry lived here and was untested. RESOLVED: `nav.drillIn`/`nav.drillOut` no longer pre-resolve focus via `query focus` (provider-only); they send `{ window }` and let the server resolve the source through the same provider→kernel-slot fallback navigate uses. Dead `focusedFq()` + `query` dispatch removed; drillOut dismiss fall-through now keys on the server `moved` flag.

- [x] `crates/swissarmyhammer-focus/tests/integration/two_window_isolation.rs` (new drill tests) — They pass `focused_fq` inline, short-circuiting `resolve_drill_source`. RESOLVED: kept as kernel-property guards (still green); the real source-resolution guard is the new host-driven e2e (no inline `focused_fq`).

- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_nav_commands_e2e.rs` — The e2e's `SeedProvider::focus()` returned `SCOPE_TOP` unconditionally, so it could not expose the failure. RESOLVED: added `GapProvider` (geometry served, `focus()` → `None`) and `drill_resolves_source_via_kernel_slot_when_ui_focus_is_absent`, which drives navigate (passes) and drill (RED pre-fix: `next_fq: null`; GREEN post-fix) host-driven against the window-sensitive provider while the kernel slot is set.

### What production path still needs investigation
RESOLVED — see Fix above. The live divergence originated in the drill source-resolution (`nav.drillIn/drillOut` → `focusedFq()` → `query focus` → `provider.focus(window)` with no kernel-slot fallback). The fix routes drill's source through the server's `resolve_drill_source` (provider → kernel-slot fallback), symmetric with navigate's `resolve_nav_source`. The regression test drives host-driven drill (no inline `focused_fq`) through this path with a window-sensitive provider, fails "before" specifically on drill while navigate passes, and passes "after".

### Notes (not blockers)
- The focus-crate suite is genuinely 120/120 green and the new tests are clean, well-documented, and use the production window-rooted FQM shape + broken `window:"main"` arg correctly. They are good kernel-property guards and worth keeping — they are just not the regression guard this task requires.