---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa880
title: Perspectives can't be SELECTED — Enter arms rename instead, click doesn't activate, and there's no next/previous-perspective command
---
## What

LIVE BUG + missing capability (user-observed): you can spatially navigate BETWEEN perspective tabs, but there is **no way to select/activate a perspective**:
1. **Enter on a focused tab immediately starts the name edit** (the ScopedPerspectiveTab `app.entity.startRename` Enter carrier) instead of selecting the perspective.
2. **Clicking a tab doesn't select it** either.
3. There's **no `next perspective` / `previous perspective` command** at all.

## Expected

- **Click selects** (activates) the perspective — the bar switches the active perspective and the board re-filters.
- **Enter selects** the focused tab's perspective (primary action = activate, like every other tab UI).
- **Rename becomes a deliberate separate gesture** — move the Enter-rename carrier to a different binding (pick consistent with app conventions: e.g. vim `cw`-style or F2 and/or the tab's context menu — check what other inline renames use; document the choice). The + button's programmatic rename-arming after create (card 01KTYN8GB25ZFKSXWA0QA283PG) is unaffected — that arms rename directly, not via Enter.
- **perspective.next / perspective.previous commands** (plugin-registered in builtin/plugins/perspective-commands, keys + Navigation or View menu placement per convention, scope-appropriate availability) cycling through the perspectives visible in the active view's bar (use the shared perspectiveVisibleInView predicate).

## Root cause (found during implementation)

The perspective-commands plugin port routed `perspective.switch` / `.next` / `.prev` to the **views server's RESOLUTION ops**, which by design hold no UIState — dispatching them resolved a perspective and discarded it. The legacy Rust `SwitchPerspectiveCmd` / `NextPerspectiveCmd` / `PrevPerspectiveCmd` (which evaluate the filter and write the window's `active_perspective_id` + `filtered_task_ids`) became unreachable after the command-service cutover. Fix: re-expose those shared command impls through the **entity server** (the board-bundle module holding both the `KanbanContext` and the shared `UIState` — same precedent as the clipboard ops) as `switch perspective` / `next perspective` / `prev perspective` ops, and reroute the plugin's three activation commands to them. Cycling now performs the full atomic switch (filter eval + `UIState::switch_perspective`), not an id-only write.

## Acceptance Criteria
- [x] Clicking a perspective tab activates that perspective (board re-filters; tab shows active state) — click already dispatched `perspective.switch`; the backend now actually writes the per-window state and emits the `PerspectiveSwitch` change
- [x] Enter on a spatially-focused tab activates it (positional `nav.drillIn` shadow → `perspective.switch`)
- [x] Rename reachable via the new deliberate gesture: F2 (cua/vim/emacs), double-click (kept), and a right-click context-menu row (`app.entity.startRename` now `context_menu: true`); Enter no longer arms rename
- [x] perspective.next / perspective.prev cycle the visible perspectives (view_id-first/kind-fallback predicate, wrap-around, <2 no-op) with keys Mod+]/Mod+[ + vim `g t`/`g Shift+T`; placed on the OS View menu (group 1)
- [x] + button flow (create → arm rename) still works (add-create-rename / add-enter suites green)

## Tests
- [x] vitest red-first: Enter activates (red: armed rename); F2 arms rename; Enter on inactive tab activates that tab; commit/escape policies preserved (perspective-tab-bar.activate-and-rename.spatial.test.tsx, 13 tests; spatial-nav-end-to-end Family 5 updated)
- [x] e2e red-first: perspective.switch writes active_perspective_id + filtered_task_ids and returns the PerspectiveSwitch change; next/prev cycle with wrap-around; single-perspective no-op; View-menu placement pinned (builtin_perspective_commands_e2e — no new command ids, so full_baseline id set unchanged and green)
- [x] tsc --noEmit clean; scoped vitest 27 files / 357 tests green; cargo nextest -p swissarmyhammer-command-service 146/147 (only the pre-carded meta_tree unregister.id failure)

## Constraints
- NO whole-workspace cargo build/clippy; no kanban-app crate compile. Never touch .kanban/actors/wballard.jsonl.
- Commands-in-rust; metadata-driven; reuse existing dispatch/scope machinery.
- crates/swissarmyhammer-ui-state untouched (used only its existing `UIState::switch_perspective` model API via the kanban command layer).

## Workflow
- /tdd.

## Review Findings (2026-06-13 07:00)

### Blockers
- [x] `crates/swissarmyhammer-entity-mcp/tests/integration/meta_snapshot.rs:38` — The new `switch`/`next`/`prev perspective` ops were added to `ENTITY_OPERATIONS` (operations.rs) — which drives the entity tool's inputSchema `op` enum — but the `expected` (noun, verb, op) table in `entity_tool_meta_operations_tree_is_complete` was NOT updated to include them. Verified failing: `cargo nextest run -p swissarmyhammer-entity-mcp` → 15 passed, 1 FAILED with `inputSchema op enum must match the _meta tree's op strings` (left has the 3 new ops, right does not). The test's own comment mandates updating this snapshot "in the same PR as the operation struct change". This is a real regression introduced by this change, and directly contradicts the Tests box claim of an entity-mcp-green run (which was never asserted in the description but is implied by "the new ops' home"). Fix: add the three tuples — `("perspective","switch","switch perspective")`, `("perspective","next","next perspective")`, `("perspective","prev","prev perspective")` — to the `expected` table, then re-run to green. **RESOLVED 2026-06-13: ran the test first to confirm RED (15 passed / 1 FAILED, enum-mismatch left={next,prev,switch perspective} vs right=∅); added the three tuples to the `expected` table; re-ran → 16/16 green.**

### Warnings
- [x] `crates/swissarmyhammer-views/src/operations.rs:341` (and `:363`, `:412`) — The views server's `NextPerspective`/`PrevPerspective`/`SwitchPerspective` RESOLUTION-only ops now have ZERO production consumers. After this change, the perspective-commands plugin routes `next`/`prev`/`switch` to the `entity` server; only `goto` still routes to `views` (and `handle_goto` does its own resolution — it does not reuse `handle_switch`). The only remaining references to these three views ops are the views crate's own tests (`tests/integration/meta_snapshot.rs`, `tests/integration/views_e2e.rs`). The root-cause analysis explicitly identified these as the broken/unreachable path, yet they were left in place with no removal and no cleanup card. This is an orphaned MCP surface and a confusing duplication: two tools (`views` and `entity`) now expose identically-named `next/prev/switch perspective` ops with different semantics (resolve vs. activate). Either delete the three dead views ops (and their now-self-referential tests) or add a card + an in-code note documenting why the resolution-only ops are retained. **RESOLVED 2026-06-13: DELETED per no-dead-code. Verified first that nothing else routes to views switch/next/prev — repo-wide rg for `views.perspective.(switch|next|prev)` across TS/TSX/Rust returned ONLY the views crate's own tests; the plugin's nav.ts routes those three to `entity.perspective.*` and only `goto` to views. Removed: the `NextPerspective`/`PrevPerspective`/`SwitchPerspective` structs + their slice entries in operations.rs; the `handle_next`/`handle_prev`/`handle_switch` handlers + the now-orphaned shared `cycle` body + the 3 dispatch arms + the 3 imports in server.rs (`perspective_to_json`/`perspective_belongs_to_view`/`get_by_id` kept — still used by `handle_goto` and others); the next/prev/switch assertions in views_e2e.rs (rewrote `nav_next_prev_goto_switch` → `nav_goto_resolves_by_id`, deleted `nav_next_noop_with_single_match`); the 3 views meta_snapshot tuples (18→15 ops). Added an in-code note in both operations.rs and server.rs pointing activation to the entity tool. `cargo nextest run -p swissarmyhammer-views` → 80/80 green.**

### Verification notes (no action — recorded for the next reviewer)
- Architecture call SOUND: entity-mcp `build_perspective_command_context` mirrors the confirmed `build_clipboard_command_context` precedent; `window:<label>` moniker required with an explicit error (no silent main fallback), consistent with the ui-state per-window hardening (commit 16d5c3b7c).
- Enter re-map SOUND: `ScopedPerspectiveTab` registers a keyless positional `nav.drillIn` shadow (Enter → activate via `perspective.switch`) alongside `app.entity.startRename` rebound to F2 in all three keymaps; `keybindings.test.ts` pins both `Enter → "nav.drillIn"` (id unchanged) and `F2 → "app.entity.startRename"`. Catalogue mirror in `app-shell-commands/commands/ui.ts` carries F2 + `context_menu: true` + `scope: ["entity:perspective"]`. + button `useArmRenameOnArrival` suite green (perspective-tab-bar.add-create-rename.test.tsx).
- vim "g t" — NO collision. It shares only the `g` PREFIX with nav-commands' `g g` (nav.first). `keybindings.ts` is prefix-aware (strict-prefix detection + pending chord buffer; the code explicitly enumerates the coexisting `g g`/`g t`/`g Shift+T`/`d d` chords). Deterministic — the second keystroke disambiguates.
- next/prev cycle semantics SOUND: `cycle_perspective` uses modular wrap-around both directions, `<2` matching → `change: null` no-op, and `perspective_belongs_to_active_view` (view_id-first / kind-fallback). The fix routes cycling through the shared `switch_to_perspective` so `filtered_task_ids` is re-evaluated (not a stale id-only write). e2e pins all of this against the real plugin→entity path.
- PerspectiveSwitch propagation pinned: `perspective_switch_activates_the_perspective_for_the_window` asserts both the actual `ui_state.active_perspective_id("main")` write and the `change["PerspectiveSwitch"]` at `structuredContent.result.structuredContent.change` (the exact location the host unwraps).
- Red-green probe performed and RESTORED: rerouting `perspective.next` back to `views.perspective.next` made `perspective_next_prev_cycle_visible_perspectives_with_wraparound` go RED (active id stayed p1 — resolution-only, no UIState write); restoring to `entity.perspective.next` returns to green. `nav.ts` diff stat unchanged after restore.
- Honesty: command-service 146/147 claim verified — the single failure is `meta_tree::meta_tree_id_param_is_required_where_expected` (`unregister.id required flag`), unrelated to perspectives (pre-existing/carded). tsc clean. Scoped vitest suites green (perspective-tab-bar activate-and-rename + add-create-rename + add-enter + keybindings + perspective-context = 160; context-menu + spatial-nav-end-to-end + perspective-tab-bar = 58).

## Review Findings (2026-06-13 12:15)

Iteration-2 re-review — verified the two prior findings are GENUINELY FIXED IN CODE (not merely claimed), and that the deletion introduced no new breakage. All evidence run fresh this session.

Prior-finding verification (both confirmed flipped in code):
- BLOCKER (meta_snapshot table): the three tuples `("perspective","switch","switch perspective")`, `("perspective","next","next perspective")`, `("perspective","prev","prev perspective")` are literally present in `crates/swissarmyhammer-entity-mcp/tests/integration/meta_snapshot.rs` and match the registered `SwitchPerspective`/`NextPerspective`/`PrevPerspective` structs in `operations.rs` (ENTITY_OPERATIONS). Three-way enum↔tree↔expected aligns. `cargo nextest run -p swissarmyhammer-entity-mcp` → **16/16 passed, 0 failed** (incl. `entity_tool_meta_operations_tree_is_complete`).
- WARNING (orphaned views ops): zero `SwitchPerspective`/`NextPerspective`/`PrevPerspective` references remain anywhere in the views crate; structs/slice-entries/handlers/cycle body/dispatch arms/imports/dead e2e tests all gone; `GotoPerspective` + `handle_goto` + goto dispatch arm intact and still consumed (`nav.ts` routes goto → views); kept helpers `perspective_to_json`/`perspective_belongs_to_view`/`get_by_id` still used (no dangling imports). VIEWS_OPERATIONS = 15 ops; meta_snapshot asserts `expected.len() == 15`. `cargo nextest run -p swissarmyhammer-views` → **80/80 passed, 0 failed**.

Regression check (no change to iteration-1-verified routing): `builtin/plugins/perspective-commands/commands/nav.ts` — `perspective.next` → entity, `perspective.prev` → entity, `perspective.switch` → entity, `perspective.goto` → views. Unchanged. `cargo nextest run -p swissarmyhammer-command-service` → **146/147**, sole failure `meta_tree::meta_tree_id_param_is_required_where_expected` (pre-existing/carded) — no new failure.

### Nits
- [x] `crates/swissarmyhammer-views/src/operations.rs:18` — Stale module doc left by the deletion: "The eighteen operations group into six sub-domains" and the **nav** line "(`next`, `prev`, `goto`, `switch`)" — but the slice now holds 15 ops and nav exposes only `goto`. Update the count to "fifteen" and trim the nav line to "(`goto`)" so the doc matches VIEWS_OPERATIONS.
- [x] `crates/swissarmyhammer-views/tests/integration/meta_snapshot.rs:5` — Stale doc comment "All 18 operations are pinned." while the test body correctly asserts `expected.len() == 15`. Update the comment to 15 so the doc and the assertion agree.