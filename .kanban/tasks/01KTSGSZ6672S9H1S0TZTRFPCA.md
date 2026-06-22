---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvr3xrved49r2ehb59vgycmq
  text: |-
    Picked up. Research findings so far:

    LIVE PATH (item 1): menu.rs `open_and_notify` dispatches `file.switchBoard` via `dispatch_command_internal` → CommandService → TS plugin builtin/plugins/file-commands/index.ts → window MCP `board.switch` → window-service `handle_switch_board` (service.rs) which returns `{ ok:true, path }` and runs the board lifecycle inside `shell.switch_board` (wraps AppState::open_board). NO BoardSwitch key on the live path.

    Item 1 fix target: menu.rs:830 `source_window_label.unwrap_or("main")` builds synthetic `["window:main"]`. Will resolve a real focused/open window instead of assuming "main".

    Item 2 (BoardSwitch/BoardClose handlers in commands.rs handle_board_switch_result/handle_board_close_result): these key on result.get("BoardSwitch")/("BoardClose"). The ONLY non-test producers of those keys are the legacy Rust Command impls file_commands::SwitchBoardCmd/CloseBoardCmd (and drag_commands DragStart etc.), which are only inserted into register_commands() — the legacy command_impls map. dispatch_via_service (commands.rs:1334) confirms Stage 4 retired the command_impls fallback; CommandService is the SOLE dispatch path. So the handlers appear dead on the live path. Awaiting full reference trace (explore agent running) before deleting; also note ui_state_change_kind board_switch/board_close branches + maybe_rebuild_menu_after_cmd + tests at commands.rs:3276-3298 reference these shapes.

    Item 3 (kanban-crate `window_label_from_scope().unwrap_or("main")`): ~20 sites in file/app/ui/perspective_commands.rs Command::execute bodies. NOTE drag_commands.rs:326 is a DIFFERENT pattern (`args.sourceWindowLabel`.unwrap_or("main"), an arg-default, not scope resolution) — out of task scope. perspective_commands.rs has LIVE public utilities switch_to_perspective + evaluate_perspective_filter (evaluate_perspective_filter is used by apps/kanban-app/src/watcher.rs) which MUST be preserved.
  timestamp: 2026-06-22T16:51:53.838986+00:00
- actor: claude-code
  id: 01kvr418kw1g5fszb33van39d2
  text: |-
    PROOF OF DEAD complete (full reference trace):

    register_commands() has ZERO production callers — all ~70 calls are in #[cfg(test)] mods; no pub use re-export. Both live commands_for_scope* sites (menu.rs:581, commands.rs:2329) pass HashMap::new() empty impls. Sole dispatch is CommandService. => the legacy command_impls registry is dead in production.

    Item 2 — handle_board_switch_result/handle_board_close_result are DEAD: the only producers of {BoardSwitch}/{BoardClose} result keys are the legacy file_commands Command impls, reachable only via the dead register_commands(). Live file.switchBoard/closeBoard return {ok,path} (window-service service.rs handle_switch_board/close_board). Side-effects ACCOUNTED FOR: the window-service shell closures (main.rs:238-255) run state.open_board/close_board; set_window_board + title live in the live window-creation/dispatch path (commands.rs:964 create_window_impl, apply_board_title). board-changed emit + menu rebuild on switch currently fire ONLY inside the dead handlers + the dead BoardSwitch/BoardClose branches of maybe_rebuild_menu_after_cmd / ui_state_change_kind — so they never fire on the live path today; menu rebuild on board switch already comes from frontend app.setFocus on window mount. => removing the board branches loses nothing live.
    PLAN item 2: delete handle_board_switch_result, handle_board_close_result, drop_or_detach_board, close_or_retitle_window + their calls in apply_post_command_side_effects, the board_switch/board_close arms of ui_state_change_kind, the BoardSwitch/BoardClose conditions in maybe_rebuild_menu_after_cmd, and the 4 dead tests (ui_state_change_kind_board_switch/board_close + any handler tests). PRESERVE update_window_title (callers at 917/2174) and rebind_window_forwarder (pub(crate), other callers).

    Item 3 — DECISION: do NOT rip out register_commands() wholesale. It is dead-in-prod but (a) still threaded through the scope_commands signature the two live empty-map callers use, and (b) perspective_commands.rs hosts LIVE direct-import consumers in swissarmyhammer-entity-mcp/src/server.rs (SwitchPerspectiveCmd@615, SetFilterAndRefreshCmd@720, + imports of Delete/Next/PrevPerspectiveCmd) and the live utility evaluate_perspective_filter (watcher.rs:755/785). Wholesale deletion is a large out-of-scope refactor touching live entity-mcp code. Per task's bounded option ("or at least their silent fallbacks so they cannot be resurrected"), item 3 fix = remove the silent ctx.window_label_from_scope().unwrap_or("main") fallbacks in the Command::execute bodies (file/app/ui/perspective_commands), replacing with proper Option handling so the bug can't be resurrected. drag_commands.rs:326 is OUT of scope (args.sourceWindowLabel default, not scope resolution).

    Item 1 — fix open_and_notify to resolve a real window, not assume "main".
  timestamp: 2026-06-22T16:53:48.284686+00:00
- actor: claude-code
  id: 01kvr5c4tabeje2nzsm1vx692v
  text: |-
    DONE — all three items resolved. Evidence + actions:

    ITEM 1 (LIVE, menu.rs open_and_notify): replaced `source_window_label.unwrap_or("main")` + synthetic `["window:main"]` with `resolve_target_window(app, source)`. New pure helper `pick_target_window(source, source_is_open, focused, any_open)` (unit-tested, no Tauri runtime) resolves: still-open source → focused window → any open window → None. When None, open_and_notify dispatches `file.switchBoard` WITHOUT a `window:` moniker (the live path derives board identity from `path`) and broadcasts `board-opened` globally — never assumes a "main" window exists. 4 new unit tests (pick_target_window_*) all green.

    ITEM 2 (BoardSwitch/BoardClose handlers) — PROVEN DEAD, removed:
    Proof: (a) dispatch_via_service (commands.rs) is the SOLE dispatch path; Stage 4 retired the command_impls fallback. (b) Live `file.switchBoard`/`file.closeBoard` route TS plugin (file-commands/index.ts) → window MCP `board.switch`/`board.close` → window-service handle_switch_board/handle_close_board (service.rs:154,160) which return `{ok:true, path}` with NO BoardSwitch/BoardClose key. (c) The ONLY producers of those keys are the legacy Rust file_commands::SwitchBoardCmd/CloseBoardCmd, reachable only via register_commands(), which has ZERO production callers (every register_commands() call is in #[cfg(test)] — verified by full-repo grep). So handle_board_switch_result/handle_board_close_result (guarded on result.get("BoardSwitch")/("BoardClose")) could NEVER fire on the live path.
    Removed: handle_board_switch_result, handle_board_close_result, drop_or_detach_board, close_or_retitle_window, their two calls in apply_post_command_side_effects, the board_switch/board_close branches in ui_state_change_kind, the BoardSwitch/BoardClose conditions in maybe_rebuild_menu_after_cmd (dropped its now-unused `result` param), and the two ui_state_change_kind_board_switch/close tests.
    Side-effect accounting: live switch_board callback (main.rs:238) calls ONLY state.open_board — confirmed. The deleted handler's side effects: (i) board lifecycle/open_board now lives in shell.switch_board (window-service); (ii) set_window_board for in-place switch is driven frontend-side (window-container.tsx persistActiveBoard dispatches file.switchBoard with windowLabel) and on window creation (create_window_impl); (iii) update_window_title still happens via refresh_board_window_titles (live, flush_and_sync_after_command) + create_window_impl::apply_board_title — update_window_title retained (2 live callers); (iv) rebind_window_forwarder became fully orphaned (compiler-confirmed dead) → DELETED; forwarder (re)binding is frontend-driven via mcp_subscribe→bind_window_forwarder (idempotent per (label,board)). Fixed the now-stale doc comments referencing rebind_window_forwarder in both commands.rs and window-container.tsx. NOTE: the deleted handler was already a no-op on the live path before deletion, so removal changes NO live behavior.

    ITEM 3 (kanban-crate window_label_from_scope().unwrap_or("main")) — PROVEN DEAD (register_commands has zero prod callers), fallbacks REMOVED via the canonical window_label_required() helper (context.rs, which errors MissingScope instead of silently using "main"). Converted all ~17 scope-resolution sites in ui_commands.rs(9), app_commands.rs(3), perspective_commands.rs(6 execute/fn bodies via `?`; available()/persist fail-closed via Option), file_commands.rs CloseBoardCmd(1). PRESERVED: pub utilities switch_to_perspective + evaluate_perspective_filter (latter used by apps/kanban-app/src/watcher.rs). Updated the dead impls' tests (mod.rs + perspective_commands + file_commands test helpers: ctx_with auto-appends window:main; switch_ctx/make_ctx* seed window:main) — 1413/1413 kanban tests green.
    Two `unwrap_or("main")` remain in the crate but are OUT OF SCOPE & dead: drag_commands.rs:326 (sourceWindowLabel ARG default) and file_commands.rs:45 (windowLabel ARG default in dead SwitchBoardCmd) — both ARG-defaults, not window_label_from_scope resolution, and both in dead command impls. The only other matches are doc comments (menu.rs, context.rs).

    GATES (actual): cargo nextest run -p swissarmyhammer-kanban → 1413 passed, 0 failed. cargo check -p kanban-app → exit 0, no warnings. cargo clippy --no-deps -p swissarmyhammer-kanban -- -D warnings → clean. kanban-app menu:: tests → 18 passed. cargo fmt --check → exit 0. Also fixed pre-existing test-build breakage in command_services.rs (4 CommandMetadata literals missing the new `applies_to` field) so the kanban-app test build compiles. Used CARGO_TARGET_DIR=target/iso-ztrfpca (gitignored).
  timestamp: 2026-06-22T17:17:13.418668+00:00
- actor: claude-code
  id: 01kvr5mcr8fmgjjxxeyahqk1tm
  text: |-
    VERIFICATION (independent re-check of the final tree, gates run by me):
    - cargo nextest run -p swissarmyhammer-kanban → 1413 passed, 0 failed.
    - cargo check -p kanban-app → exit 0, no warnings.
    - cargo clippy --no-deps -p swissarmyhammer-kanban -- -D warnings → clean.
    - cargo fmt --check → exit 0.
    - kanban-app menu::/commands:: tests → 52 passed (incl. new pick_target_window_* unit tests).
    - entity-mcp compiles; live perspective consumers (server.rs) intact.

    PRE-EXISTING (not this task): swissarmyhammer-entity-mcp tests kanban_op_surface_is_frozen + kanban_meta_operations_tree_is_frozen FAIL — confirmed they also fail on a clean `git worktree` at HEAD (the frozen surface at HEAD lacks `search tasks` + comment ops). Orthogonal to window-fallback work; my diff touches no kanban operation/meta definitions.

    ACCEPTANCE GREP `rg 'unwrap_or("main")'` (excluding .kanban): 3 doc-comment mentions + 1 test doc comment (not code); 2 genuinely-dead arg-default code sites — drag_commands.rs:326 (args.sourceWindowLabel default) and file_commands.rs:45 (SwitchBoardCmd windowLabel arg default), both in command impls reachable ONLY via the test-only register_commands() and both arg-defaults, NOT scope-chain window resolution. NO live window-resolution unwrap_or("main") remains.

    What changed:
    - Item 1: menu.rs open_and_notify → resolve_target_window/pick_target_window (open source → focused → any open → None; None ⇒ dispatch file.switchBoard with no window: moniker + global board-opened emit). Never assumes "main".
    - Item 2: deleted handle_board_switch_result/handle_board_close_result/drop_or_detach_board/close_or_retitle_window + the now-orphaned rebind_window_forwarder, plus the BoardSwitch/BoardClose branches in ui_state_change_kind and maybe_rebuild_menu_after_cmd, plus 2 dead tests. Side-effects accounted: board lifecycle lives in window-service shell.switch_board/close_board (state.open_board/close_board); set_window_board on the live path is create_window_impl + frontend; titles via refresh_board_window_titles (update_window_title retained, live callers at 917/2174); forwarder binding is frontend mcp_subscribe-driven and idempotent (so rebind_window_forwarder removal loses nothing live).
    - Item 3: ~17 ctx.window_label_from_scope().unwrap_or("main") → ctx.window_label_required()? (new helper, CommandError::MissingScope); available()/persist paths fail-closed/skip. PRESERVED switch_to_perspective + evaluate_perspective_filter (watcher.rs) and the live perspective Command impls (entity-mcp), whose upstream build_perspective_command_context already requires a window: moniker so window_label_required never trips on the live path. Also fixed a pre-existing kanban-app test-build gap (command_services.rs CommandMetadata literals missing the new applies_to field).

    PROCESS NOTE: a context-inheriting fork I spawned mid-task autonomously completed the same plan in parallel and moved the card to Review; the final tree is its (equivalent, slightly cleaner) implementation. I independently re-verified all gates above rather than trusting its report. Ran the double-check adversarial gate from the main context (the fork couldn't spawn it). Isolated target dir target/iso-ztrfpca cleaned up.
  timestamp: 2026-06-22T17:21:43.688422+00:00
- actor: claude-code
  id: 01kvr5wjmhehg9y2dbpp4bb60t
  text: 'double-check (adversarial) verdict: REVISE with a SINGLE doc-only finding — a stale comment in file_commands.rs (test mod) still referenced the deleted `handle_board_switch_result`. FIXED: reworded to describe the idempotent re-issue invariant without naming the removed handler. All five disproof targets came back clean: rebind_window_forwarder is truly orphaned (its 3 callers were inside the deleted dead handlers; it never ran on the live path even at HEAD), board handlers truly dead, perspective hardening has no live regression (entity-mcp build_perspective_command_context already requires a window: moniker), open_and_notify None-branch is correct, no orphaned imports/dead code. Functional change is PASS-equivalent; the sole finding was the stale doc, now resolved. cargo fmt --check still exit 0 after the comment edit. Task remains in Review, ready for /review.'
  timestamp: 2026-06-22T17:26:11.857293+00:00
- actor: claude-code
  id: 01kvr7ydcdd8n1cv190m4wd0dc
  text: |-
    Addressed the 3 review-findings checklist items (1 warning + 2 nits) and flipped all checkboxes (incl. the 3 acceptance criteria, now all met).

    CHANGES (my session only):
    - file_commands.rs `SwitchBoardCmd::execute`: replaced the `windowLabel`/`window_label` arg `.unwrap_or("main")` with `ctx.window_label_required()?`, mirroring its twin `CloseBoardCmd` exactly (fail-closed on a missing `window:<label>` scope moniker, no silent "main"). Updated the doc comment.
    - drag_commands.rs `resolve_drag_start_params`: replaced the `sourceWindowLabel` arg `.unwrap_or("main")` with `ctx.window_label_required()?` for consistency (no `unwrap_or("main")` facade left to resurrect). Updated doc comments.
    - perspective_commands.rs (nits): collapsed the 4 near-verbatim test helpers (make_ctx / make_ctx_with_scope / make_ctx_with_ui / make_ctx_with_scope_and_ui) into ONE parameterized `fn make_ctx(kanban, args, scope_chain: Option<Vec<String>>, ui: Option<Arc<UIState>>)` and updated all ~30 call sites; extracted `const DEFAULT_TEST_WINDOW: &str = "main"` and built the moniker from it in `with_default_window()`.

    TEST-CALLER UPDATES (the hardening's only fallout, all legacy/test-only callers):
    - file_commands.rs SwitchBoardCmd tests: now seed `window:main` in scope; renamed `switch_board_cmd_uses_explicit_window_label` -> `_uses_scope_window_label`; added `switch_board_cmd_missing_window_label_returns_error` (genuine fail-closed assertion).
    - drag_commands.rs: helper `ctx_with_args_and_ui` now injects `window:main`; added `ctx_with_args_scope_ui`; `drag_start_custom_source_window_label` now sets the window via scope (not arg); added `drag_start_without_window_moniker_fails_closed`; renamed `_default_source_window_label_is_main` -> `_resolves_source_window_label_from_scope`.
    - commands/mod.rs register_commands() drag.start tests: all 8 now pass `vec!["window:main"]` scope so they still assert their intended condition (e.g. missing taskId fails on taskId, not on window).

    RG RESULT (acceptance grep): `rg 'unwrap_or("main")' crates/swissarmyhammer-kanban apps/kanban-app` -> only 2 hits, both DOC COMMENTS (menu.rs describing the prior fix, context.rs describing what callers must NOT do). NO code-level window/arg `unwrap_or("main")` fallback remains.

    GATES (actual): `cargo nextest run -p swissarmyhammer-kanban` = 1415 passed / 0 skipped; `cargo clippy --no-deps -p swissarmyhammer-kanban -- -D warnings` = clean (exit 0); `cargo check -p kanban-app` = exit 0; `cargo fmt` applied.

    DOUBLE-CHECK (advisory gate) returned REVISE with one finding: app_commands.rs / ui_commands.rs carry ~13 functional `window_label_from_scope().unwrap_or("main")` -> `window_label_required()?` conversions (plus a fail-closed `StartRenamePerspectiveCmd::available()`) that are real uncommitted edits, not fmt noise. JUSTIFICATION FOR PROCEEDING: those edits are NOT from this session — they were already in the working tree at session start (the dead/silent-fallback removal across all five `*_commands.rs` files, which is card item 3 verbatim and already covered by this card's own "Verified safe / TREE CONSISTENCY" review section). My session scope was strictly the warning + 2 nits and touched only file_commands.rs, drag_commands.rs, perspective_commands.rs, and commands/mod.rs. The critic's concern was a commit-message-hygiene point premised on my having authored those files; I did not. cargo fmt reformatted the whole tree (a required gate), which is why those pre-existing edits appear in the same working diff. Risk is low (the critic agreed: ui-state service already requires `window:` monikers, production dispatch always supplies one, `cargo check -p kanban-app` green). Proceeding to review.
  timestamp: 2026-06-22T18:02:09.165006+00:00
- actor: claude-code
  id: 01kvr8s76pxrn6prtk40zk1evw
  text: |-
    Addressed both 2026-06-22 13:03 test-hygiene warnings — consolidated `DEFAULT_TEST_WINDOW` to a single shared definition.

    CHANGE:
    - Added `pub const DEFAULT_TEST_WINDOW: &str = "main"` + `pub fn default_window_moniker() -> String` to the existing crate-level test-support module `crates/swissarmyhammer-kanban/src/test_support.rs` (gated `#[cfg(any(test, feature = "test-support"))]` at lib level — does not leak into production builds). This is the established shared test-helper home (already used via `use crate::test_support::...` across the crate's test modules; its own docs call it "the single source of truth" for previously-duplicated helpers).
    - Removed the duplicate `const DEFAULT_TEST_WINDOW` from `commands/drag_commands.rs` and `commands/perspective_commands.rs`.
    - Removed the inline `"window:main"` literal from the `ctx_with` helper in `commands/mod.rs`.
    - All three test modules now reference the shared definition: `ctx_with` (mod.rs) and `with_default_window` (perspective_commands.rs) append `default_window_moniker()`; `ctx_with_args_and_ui` (drag_commands.rs) uses `default_window_moniker()`.

    No production code touched; no new constant introduced in more than one place. Per-test-body `"window:main"` call-site literals (args passed to the helpers) were left as-is — they are not the helpers the findings named and are out of scope.

    rg RESULTS (actual):
    $ rg 'DEFAULT_TEST_WINDOW' crates/swissarmyhammer-kanban
      src/test_support.rs:68:pub const DEFAULT_TEST_WINDOW: &str = "main";   <- the ONE const def
      src/test_support.rs:70:/// Build the `window:<DEFAULT_TEST_WINDOW>` scope moniker ...   (doc)
      src/test_support.rs:73:    format!("window:{DEFAULT_TEST_WINDOW}")   (helper body)
      src/commands/perspective_commands.rs:2227:/// Append a `window:<DEFAULT_TEST_WINDOW>` moniker ...   (doc)
      src/commands/mod.rs:362:/// (`crate::test_support::DEFAULT_TEST_WINDOW`) is appended ...   (doc)

    $ rg 'const DEFAULT_TEST_WINDOW' crates/swissarmyhammer-kanban
      src/test_support.rs:68:pub const DEFAULT_TEST_WINDOW: &str = "main";   <- exactly ONE

    No `"window:main"` inline literal remains in the drag/perspective test helpers.

    GATES (all green):
    - cargo nextest run -p swissarmyhammer-kanban: 1415 passed, 0 failed (1 leaky)
    - cargo clippy --no-deps -p swissarmyhammer-kanban --tests -- -D warnings: exit 0, clean
    - cargo check -p kanban-app: exit 0
    - cargo fmt: applied, no further changes
  timestamp: 2026-06-22T18:16:47.574190+00:00
- actor: claude-code
  id: 01kvr8sm7bw3tg9erjwv259sr6
  text: |-
    Addressed both 13:03 review findings (test-const consolidation). Single shared definition now lives in crate-level test-support: crates/swissarmyhammer-kanban/src/test_support.rs

      pub const DEFAULT_TEST_WINDOW: &str = "main";
      pub fn default_window_moniker() -> String { format!("window:{DEFAULT_TEST_WINDOW}") }

    (gated `#[cfg(any(test, feature = "test-support"))]` at the lib level — does not leak into production builds.)

    Changes:
    - Removed the duplicate `const DEFAULT_TEST_WINDOW` from drag_commands.rs (tests mod) and perspective_commands.rs (tests mod); both now `use crate::test_support::default_window_moniker`.
    - ctx_with helper in commands/mod.rs no longer hardcodes "window:main" inline — it appends default_window_moniker().
    - with_default_window (perspective_commands.rs) and ctx_with_args_and_ui (drag_commands.rs) use default_window_moniker().
    No production code touched; the already-approved hardening untouched.

    rg verification (actual):
      $ rg -n 'const DEFAULT_TEST_WINDOW' crates/swissarmyhammer-kanban
      crates/swissarmyhammer-kanban/src/test_support.rs:68:pub const DEFAULT_TEST_WINDOW: &str = "main";
      -> exactly ONE const definition.

      $ rg -n 'DEFAULT_TEST_WINDOW' crates/swissarmyhammer-kanban
      test_support.rs:68 (def), test_support.rs:70 (doc), test_support.rs:73 (helper body),
      perspective_commands.rs:2227 (doc comment), commands/mod.rs:362 (doc comment)
      -> all non-def mentions are doc comments or the single helper body.

      $ rg -n '"window:main"' (perspective_commands.rs, drag_commands.rs helpers) -> none in the test helpers.

    Gates (all green):
    - cargo nextest run -p swissarmyhammer-kanban -> 1415 passed, 0 failed (1 leaky), 0 skipped
    - cargo clippy --no-deps -p swissarmyhammer-kanban --tests -- -D warnings -> exit 0, clean
    - cargo check -p kanban-app -> exit 0 (Finished)
    - cargo fmt -> applied, no further changes

    Task remains in `review` column.
  timestamp: 2026-06-22T18:17:00.907178+00:00
- actor: claude-code
  id: 01kvr8vrdcn1cmbw5xv682xnwz
  text: |-
    really-done adversarial double-check ran; verdict REVISE, but its two blocking findings (#3 "production code touched", #5 "changes outside test modules") are about the PRE-EXISTING already-approved per-window hardening (window_label_required in commands_core/context.rs and the production unwrap_or("main")->window_label_required()? call-site conversions in app_commands.rs / ui_commands.rs / file_commands.rs / drag_commands.rs / perspective_commands.rs), NOT this consolidation change.

    Verified the attribution: the critic reviewed the whole uncommitted diff (git diff HEAD) which already carried that hardening before this session. My session's edits are confined to test-only code:
      git diff HEAD -- commands_core/context.rs | grep -E 'DEFAULT_TEST_WINDOW|default_window_moniker'  -> NONE
      git diff HEAD -- app_commands.rs ui_commands.rs | grep -E 'DEFAULT_TEST_WINDOW|default_window_moniker'  -> NONE
    So the production-code delta the critic flagged contains none of my const work — it is the earlier hardening already cleared by the 12:32 review ("Verified safe: ... window_label_required() ... the new error path does not trip in production"; "SCOPE ... minimal and legitimate").

    The critic's clean findings #1/#2/#4 cover exactly my change (single const definition, test-gated, all three modules reference it) and all PASS. Proceeding past the advisory #3/#5 with this justification per really-done's contract: they are out of this card's scope (the card explicitly said "Do NOT touch production code or the already-approved hardening") and were already reviewed/approved.

    Both 13:03 review findings remain resolved; task stays in `review`.
  timestamp: 2026-06-22T18:18:10.732604+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffdd80
project: ui-command-cleanup
title: Audit app-side "main" window fallbacks outside the ui_state op path (menu.rs open_and_notify, dormant BoardSwitch/BoardClose handlers, legacy kanban commands module)
---
## What
Follow-up from the per-window hardening card `01KTECWA8D05FVKJ80MA3H0FFY` (which removed the silent `unwrap_or("main")` from the `ui_state` per-window mutation ops). The fallback inventory found three RESIDUAL `"main"` defaults outside that path:

1. **LIVE**: `apps/kanban-app/src/menu.rs` `open_and_notify` — `source_window_label.unwrap_or("main")` when `focused_window_label(app)` returns `None` (File > Open Board with no focused window). It then builds a synthetic scope chain `["window:main"]`. Since windows are created dynamically (`main.rs`: no static "main" window), this can target a nonexistent window. Should resolve a real window (e.g. any open window) or surface an error, not assume "main".
2. **DORMANT**: `apps/kanban-app/src/commands.rs` `handle_board_switch_result` / `handle_board_close_result` — read `window_label` from a `BoardSwitch` / `BoardClose` result key with `.unwrap_or("main")`. The live window-service `switch board` / `close board` ops return `{ok, path}` WITHOUT those keys (the shapes come from the retired legacy Rust `file_commands`), so these handlers appear to never fire on the live TS-plugin path. Verify dead, then delete the handlers (and decide where their side effects — `set_window_board`, forwarder rebind, title update — actually live now, since the shell `switch_board` callback only calls `state.open_board`).
3. **DEAD**: `crates/swissarmyhammer-kanban/src/commands/{ui,app,file,drag,perspective}_commands.rs` — ~20 `ctx.window_label_from_scope().unwrap_or("main")` sites in the legacy Rust `Command` impls retired by the Stage 4 cutover (`state.command_impls` deleted). Only utility fns (e.g. `evaluate_perspective_filter`) are still referenced. Delete the dead command impls or at least their silent fallbacks so they cannot be resurrected with the bug intact.

## Acceptance Criteria
- [x] `open_and_notify` no longer assumes a "main" window exists when no window is focused.
- [x] The dormant `BoardSwitch`/`BoardClose` handlers are confirmed dead and removed (or confirmed live and hardened), with their side-effect responsibilities accounted for.
- [x] No remaining `unwrap_or("main")` window resolution in live code paths. #tech-debt

## Review Findings (2026-06-22 12:32)

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/commands/file_commands.rs:45` (`SwitchBoardCmd::execute`) — inconsistent half-application of the hardening. This file WAS edited by this diff: `CloseBoardCmd::execute` was correctly converted to `window_label_required()?`, but its twin `SwitchBoardCmd::execute` in the same file still reads the `windowLabel`/`window_label` ARG with `.unwrap_or("main")` and then `ui.set_window_board("main", path)`. NOT a live-path violation: `register_commands()` (`commands/mod.rs`) — the only thing that instantiates `SwitchBoardCmd` — has NO production caller (every call site is `#[cfg(test)]` or in `tests/`; live dispatch goes through `CommandService` per `apps/kanban-app/src/commands.rs:1321-1333`, "sole dispatch path... the legacy `command_impls` fallback was retired in Stage 4"). So this is a legacy/test-only in-process facade, not the runtime path. But it is exactly the "resurrected with the bug intact" hazard the card warns about, and hardening one twin while leaving the other is internally inconsistent. Harden `SwitchBoardCmd` to match `CloseBoardCmd`, or delete the dead facade. (Same applies to `drag_commands.rs:326` `resolve_drag_start_params` `sourceWindowLabel.unwrap_or("main")` — pre-existing, NOT touched by this diff, same legacy-only path; the card body named `drag_commands.rs`, so decide in-scope vs follow-up.)

### Nits (engine, tied to this diff — test quality)
- [x] `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs` (`make_ctx_with_scope_and_ui`, `make_ctx_with_ui`, `make_ctx_with_scope`, `make_ctx`) — four near-verbatim test helpers differing only in which params they accept/set. Collapse into one parameterized helper `fn make_ctx(kanban, args, scope_chain: Option<Vec<String>>, ui: Option<Arc<UIState>>)` and update call sites.
- [x] `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs` (`with_default_window()`) — hardcodes `"window:main"` inline. Extract `const DEFAULT_TEST_WINDOW: &str = "main"` and build the moniker from it so the default is centralized.

### Out of scope / not flagged (per review brief)
- The engine flagged an empty `#[ignore]` test in `menu.rs` as a blocker. On inspection it is the PRE-EXISTING Stage-4 `#[ignore = "...Stage 4 cut-over"]` shell (`view_submenu_contains_ai_toggle_command`), not introduced by this `unwrap_or` task — out of scope for this card. Not carried as a finding.

### Verified safe (CRITICAL review focus — all confirmed)
- DEAD-CODE SAFETY: dead-handler deletions are clean. `handle_board_switch_result`, `handle_board_close_result`, `drop_or_detach_board`, `close_or_retitle_window`, `rebind_window_forwarder` and the `BoardSwitch`/`BoardClose` result keys have ZERO references anywhere in the tree — no orphans, no live dispatch produces those keys.
- NO WRONGFUL DELETION: `switch_to_perspective` and `evaluate_perspective_filter` are PRESERVED, still defined and still called by the live perspective `Command` impls (the path used by `swissarmyhammer-entity-mcp`).
- `window_label_required()` (`commands_core/context.rs`) returns `CommandError::MissingScope`; the live entity-mcp perspective path already supplies a `window:` moniker, so the new error path does not trip in production.
- SCOPE: `apps/kanban-app/src/command_services.rs` change is minimal and legitimate — exactly 4 `applies_to: None` additions to `CommandMetadata` TEST literals (mechanical fix for the new struct field), no new logic. Not scope creep.
- TREE CONSISTENCY: `check working` returns 0 errors / 0 warnings; no half-applied edits, no orphaned references to deleted symbols. The fork-produced final tree is internally consistent.

## Review Findings (2026-06-22 13:03)

### Warnings
- [x] `crates/swissarmyhammer-kanban/src/commands/mod.rs:1654` — Hardcoded 'window:main' literal in ctx_with helper function should use a named constant. RESOLVED: the `ctx_with` helper now appends `crate::test_support::default_window_moniker()` instead of the inline `"window:main"` literal. The default is sourced from the single shared `DEFAULT_TEST_WINDOW` const.
- [x] `crates/swissarmyhammer-kanban/src/commands/perspective_commands.rs:1168` — `DEFAULT_TEST_WINDOW` constant is duplicated across test modules. RESOLVED: the duplicate `const DEFAULT_TEST_WINDOW` definitions in `perspective_commands.rs` and `drag_commands.rs` were removed. The constant now lives in exactly ONE place — `crate::test_support` (`crates/swissarmyhammer-kanban/src/test_support.rs`, gated `#[cfg(any(test, feature = "test-support"))]`) — as `pub const DEFAULT_TEST_WINDOW: &str = "main"` plus a `pub fn default_window_moniker()` helper. All three test modules (`commands/mod.rs`, `drag_commands.rs`, `perspective_commands.rs`) reference that single definition.