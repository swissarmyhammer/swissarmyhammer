---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff980
title: Drill-in / Escape broken with two windows on the same board — window/board conflation in drill path
---
## What

LIVE BUG (user-observed): drill-in and drill-out (Escape) do not work when TWO windows have the SAME board open. Directional navigation works fine in the same configuration. Single-window drill works (the prior fix `01KTPYRQN815A4JQQSFFQK27VK` made drill source-resolution symmetric with navigate and is verified).

**Diagnosis directive from the user**: there is something conflated between 'window' and 'board' in a special case that exists for drill-in/escape but NOT for navigation. We have the fully-qualified scope chain (window-rooted FQMs, `/<window-label>/window/...`) and the drill path MUST use it — no side-channel window/board lookups, no "peeking and poking" at side fields.

The asymmetry to hunt: find every input drill consumes that navigate does not, and check each for window/board conflation. Known structural differences:
- **Drill-in needs a geometry/children snapshot** to pick the child target; navigate's directional pick also uses a snapshot but may obtain/scope it differently. Trace how drill's snapshot / scope_chain is pulled (`UiGeometryProvider::snapshot` / `scope_chain` in `crates/swissarmyhammer-focus/src/provider.rs`, used from `resolve_drill_source` / `handle_drill_in` / `handle_drill_out` in `crates/swissarmyhammer-focus/src/server.rs`) and whether any request/response is keyed or routed by something board-scoped (board id, layer name, non-window-rooted scope) instead of the window-rooted FQ chain.
- **Drill-out walks UP the scope chain** (parent resolution via `SpatialRegistry` layers / `drill_out` in the kernel). Check whether parent resolution at the board/window boundary uses a layer keyed by board or a shared key that collides when two windows register the same board's scopes. Two windows on one board: every FQM must remain window-unique (`/<labelA>/window/board:X/...` vs `/<labelB>/window/board:X/...`); any map keyed by an un-rooted segment (e.g. `board:X` or layer NAME instead of layer FQ) collides.
- **The host→UI request/reply channel** (`apps/kanban-app/src/ui_request.rs` + `apps/kanban-app/ui/src/lib/ui-request-responder.ts`): drill's provider pulls (focus / scope_chain / snapshot) must be window-scoped via `emit_to(window_label)` and the responder must answer only for its own window. If a request is broadcast (`app.emit`) or correlated by board, the WRONG window's responder can answer first when both windows show the same board — wrong geometry/scope_chain → drill misfires while navigate (different source of data) survives.
- **The nav-commands plugin** (`builtin/plugins/nav-commands/index.ts`): drillIn/drillOut send `{ window }` — verify the window value is the invoking window's label in a two-window scenario (not the board, not the "main"/first window, not a stale label).

## Acceptance Criteria
- [ ] With two windows open on the SAME board: drill-in in window A commits focus to the correct child IN window A; window B's focus is untouched
- [ ] Same for drill-out (Escape) in window A: parent focus committed in A, B untouched; works in both windows independently
- [ ] Root cause identified and fixed: the specific window/board conflation named and removed — the drill path derives the owning window exclusively from the fully-qualified scope chain (fq root segment), same as navigate
- [ ] Directional navigation and jump remain correct in the two-window-same-board configuration (no regression)
- [ ] Single-window drill (prior task's regression guard) still passes

## Tests
- [ ] A host-driven regression test that reproduces THIS configuration: TWO windows whose FQMs root at different labels but share identical board-level structure below the window root (same board segments), each with its own window-sensitive provider state. Drive drill-in and drill-out host-driven (no inline `focused_fq`, no inline snapshot if production omits it) in window A and assert: correct commit in A, zero events/slot changes in B. This must FAIL on current code (reproducing the live bug) and PASS after the fix. Location: `crates/swissarmyhammer-command-service/tests/integration/builtin_nav_commands_e2e.rs` (real plugin path) and/or `crates/swissarmyhammer-focus/tests/.../two_window_isolation.rs` — whichever layer actually reproduces it; prefer the real plugin path.
- [ ] If the conflation is in the Tauri request/reply channel (`ui_request.rs`/responder), add a test at that seam proving window-scoped routing (request for window A never answered with window B's data)
- [ ] `cargo nextest run -p swissarmyhammer-focus` and `-p swissarmyhammer-command-service` — green (the two pre-existing keybinding-metadata failures `builtin_ui_commands_e2e`/`builtin_app_shell_commands_e2e` from card 01KTPDTH772HSEV5F7R1DKYDNJ are out of scope)

## Constraints
- Do NOT run whole-workspace `cargo build`/`cargo clippy`/`cargo run` — `tauri dev` is hot-reloading and a full build races the watcher. Crate-scoped `cargo nextest run -p <crate>` is fine.
- Use the fully-qualified scope chain for window identity everywhere in the drill path. No new side fields, no special cases.

## Workflow
- Use `/tdd` — write the failing two-window-same-board drill test FIRST (it must reproduce the live bug), then fix.