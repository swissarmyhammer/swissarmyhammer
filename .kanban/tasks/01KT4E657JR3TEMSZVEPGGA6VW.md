---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd980
project: builtin-commands
title: 'Builtin command plugins don''t load: wire app/views/window servers into kanban-app bootstrap + re-discover after AppHandle exists'
---
DISCOVERED while implementing the per-window PluginHost card (01KT45WX). Verified against the current tree: **zero builtin command plugins load in production today.**

## Root cause (verified)
The kanban-app bootstrap (`install_app_command_services` in `apps/kanban-app/src/command_services.rs` + `expose_kanban_module`) exposes only: `entity`, `focus`, `store`, `ui_state`, `commands`, `kanban`, and `window` *conditionally* (`if let Some(ws) = window_shell`). `AppState::new` (`state.rs:513`) calls `wire_command_services(ui_state, None)` — window_shell is `None` — so **`window` is not exposed**, and **`app` and `views` are never exposed anywhere** (grep for `expose_rust_module("app"` / `("views"` is empty).

The 7 builtin command plugins declare their backends via `ensureServices`:
- task-commands → `[commands, kanban]` ✓ (would load)
- entity-commands → `[commands, entity]` ✓ (would load)
- perspective-commands → `[commands, views]` ✗ (views unwired)
- file-commands → `[commands, window]` ✗ (window unwired in no-AppHandle path)
- ui-commands → `[commands, ui_state, window, focus]` ✗ (window)
- kanban-misc-commands → `[commands, kanban, window, views]` ✗ (window + views)
- app-shell-commands → `[commands, app, ui_state, store]` ✗ (app)

5 of 7 fail `ensureServices` (`register` throws "unknown server"). `discover_and_load_all` is **all-or-nothing** (`crates/swissarmyhammer-plugin/src/host.rs:60-68`: any one failure → `rollback_loaded` + `return Err`), so ALL builtin plugins roll back. `AppState::new` only logs the warning (`state.rs:81-82`), so the app runs with an empty command registry → `list command` returns nothing → palette/menus have no builtin commands.

The `app`/`views`/`window` server crates EXIST and are tested in isolation (e.g. `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs` wires `app` in its harness) — this is a **wiring gap**, not missing implementation.

## The AppHandle wrinkle
`window` and `app` servers are Tauri-`AppHandle`-backed (`WindowShell` / `AppShell`). The AppHandle does not exist at `AppState::new`, so they cannot be wired there. They must be exposed at/after the Tauri `setup` hook, followed by a **re-discovery** of the builtin plugins (discovery currently runs once in `AppState::new`, before the AppHandle exists). `views` is NOT AppHandle-bound and can be wired in the no-AppHandle path immediately.

## Work
- Expose `views` in `install_app_command_services` (no AppHandle needed).
- Wire `window` + `app` (AppShell) at the setup hook where the AppHandle exists, then trigger discovery (or re-discovery) so all 7 builtin plugins load.
- Decide discovery ordering: either (a) defer the FIRST discovery until after all servers (incl. AppHandle-bound) are wired at setup, or (b) keep the early discovery and re-run after setup. Given atomic discovery, (a) is cleaner.
- Consider whether `discover_and_load_all` should stay atomic. Per design it's intentionally atomic (the "failed load" capability test). Keep atomic; fix the wiring so nothing fails. Do NOT silently make discovery best-effort — that would hide real plugin breakage.
- This interacts with the per-window host work (01KT45WX): once hosts are per-board, the per-board host build must also have app/views/window available (window/app via the shared AppShell captured at setup).

## Acceptance
- With a real (or test) AppShell wired, `discover_and_load_all` loads all 7 builtin command plugins; `list command` returns the full builtin baseline.
- `crates/swissarmyhammer-command-service/tests/baseline` catalog (62 commands) is reachable through the kanban-app host, not just the command-service test harness.
- Real-pipeline test in kanban-app: after bootstrap+setup wiring, a board's host `list command` returns the builtin commands (this is the assertion the per-window card 01KT45WX wanted but couldn't meet).

## Also fixed in passing (per-window card branch)
`extract_builtin_plugins` had a double-nesting bug: `include_dir 0.7.4`'s `Dir::extract` re-joins the bundle-name-prefixed entry path, producing `plugins/<bundle>/<bundle>/…` and ENOENT on flat bundles. The per-window card (01KT45WX) fixed extraction to write into `plugins/<bundle>/` directly. Verify that fix is retained.

## Review Findings (2026-06-02 16:14)

Overall: clean and well-reasoned. Views resolver faithfully mirrors the entity/store task-local pattern; deferred-discovery ordering is correct and applied to both global and per-board hosts; `expose_apphandle_modules` shared across both paths. No blockers; the main theme is the per-call blocking-thread pattern.

### Warnings
- [x] `apps/kanban-app/src/main.rs:47-63,86-154` — `block_on_isolated` spawns a fresh OS thread + new current-thread runtime and BLOCKS via `.join()`, invoked from synchronous `WindowShell` methods that run inside the dispatcher's async task. So every `open_new_window`/`switch_board`/`close_board`/`init_board` shell op blocks a Tokio worker on a thread-join (and `switch_board`/`close_board` re-enter `AppState` board locks while doing so — starvation/deadlock risk under concurrent dispatch). Consider making the shell seam async, or routing these onto a dedicated long-lived runtime instead of spawn+join per call.
- [x] `apps/kanban-app/src/main.rs:47-63` vs `apps/kanban-app/src/state.rs:491-548` — `block_on_isolated` and `build_board_platform`'s `spawn_blocking`+new-runtime confinement are the same "run a `!Send`/sync-bridged future off the async worker" pattern implemented two different ways (the main.rs comment says it mirrors `build_board_platform`). Extract one shared helper so the confinement strategy lives in one place and can't drift.

### Nits
- [x] `crates/swissarmyhammer-views/src/server.rs:266-275` — `perspective_context()`/`views_context()` now return `Option<Arc<…>>` (was `Arc<…>`); only caller is the single-board test path. Confirm the `Arc::ptr_eq` kernel-sharing tests were updated for the `Option`, and that the doc mentions the `None`-when-unscoped case.
- [x] `crates/swissarmyhammer-kanban/src/context.rs:59,82` — `views_arc()` returns an owned `Option<Arc<…>>` (clone) while `perspective_context_arc()` returns `Result<&Arc<…>>` (borrow); asymmetric sibling accessors force the caller (commands.rs:1364-1365) to `Arc::clone` one and use the other directly. Consider making both return owned clones for a symmetric API.
- [x] `apps/kanban-app/src/state.rs:1235-1238,54-57,307-316` — the `ApphandleShells` type alias (`Option<(Arc<dyn WindowShell>, Arc<dyn AppShell>)>`) is only used by `build_board_platform*`; `BoardHandle::open`'s param and `apphandle_shells()`'s return spell the same tuple inline. Use the alias in all three for consistency.

## Review Findings (re-review 2026-06-02 19:40)

Re-review of the fix delta surfaced one warning relating to this card.

### Warnings
- [x] `crates/swissarmyhammer-views/src/server.rs:179,187` — `ViewsServer::perspective_context()` and `views_context()` are dead: rg confirms ZERO callers anywhere in `crates/` or `apps/` (production AND tests). The only `perspective_context()` callers in the tree are `KanbanContext::perspective_context()` (a different type) and `apps/kanban-app/tests/substrate_guard.rs:86` which calls `kanban.perspective_context()` (the KanbanContext accessor) + `kanban.views()` — never the `ViewsServer` inherent methods. Their doc claimed a `Arc::ptr_eq` test that does not exist; kernel-sharing IS covered through the real resolver path via `KanbanContext` in `substrate_guard.rs`, so these accessors are not needed to keep any test alive. As `pub` inherent methods they evaded the `dead_code` lint. RESOLUTION: DELETED both accessors and their doc comments. `Arc`/`PerspectiveContext`/`ViewsContext` imports remain used by `ViewsBoardServices` and the resolver, so no unused-import fallout. Build clean (zero warnings), `cargo test -p swissarmyhammer-views` 13 passed / 0 failed.