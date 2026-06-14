---
assignees:
- claude-code
depends_on:
- 01KT4E657JR3TEMSZVEPGGA6VW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffda80
project: plugin-arch
title: 'Per-window PluginHost: AppState window→host map + dispatch resolves the calling window''s host'
---
Decision (user, multi-window discussion): each board window gets its OWN PluginHost for full registry isolation, so per-board project plugins can't leak or collide across windows. Spec: ideas/plugins/plugin-architecture.md → "Project layer is per board window".

## Current state
The kanban app is multi-window in ONE process (`window.new` builds a new `WebviewWindow` in-process, `commands.rs:833`), but there is ONE process-wide host: `AppState.plugin_platform: TokioMutex<PluginPlatform>` (`state.rs:475`), with one global ServerRegistry + one global command registry + one CommandService. Dispatch reads that single service (`commands.rs:1302` `state.plugin_platform.lock().await.command_service()`).

## Work
- Replace the single `plugin_platform` on `AppState` with a **map keyed by board window** (window label or board_dir → `PluginPlatform`). Build a host when a board window opens; tear it down when it closes or switches boards (ride alongside the existing per-board MCP server lifecycle — see `state.rs:118` "Shut the per-board MCP server down so closing a board never leaks").
- Route every command path to the **calling window's** host: `dispatch_via_service` / `try_dispatch_via_command_service` (`commands.rs:1276+`), `list command` (the palette/menu fetch), and `mcp_subscribe`/notifications. The calling `WebviewWindow` identity comes from the Tauri IPC; map it → that window's platform.
- Each per-window host wires its own command backends (`wire_command_services`) and keeps the existing `tokio::task_local!` substrate seam (`CURRENT_STORE_CTX`, `CURRENT_ENTITY_BOARD_SERVICES`) set around its dispatch — so the DATA path is unchanged; only the host + its registries become per-window.
- Builtin extraction stays a one-time shared on-disk cache (`extract_builtin_plugins`); each host discovers from that cache + the shared user `plugins/` dir, so the SOURCE is shared though isolates are per-window.

## Acceptance
- Two windows open on two different boards: each has its own host; a command/server registered by board A's project plugin is absent from board B's `list command` and palette.
- Closing board A's window tears down its host (isolates + registrations + watcher) without affecting board B.
- Builtin + user commands still appear in every window.
- Existing single-window behavior + full command baseline e2e still green.

Cost accepted by the user: N× V8 isolate floor for N open windows.

Blocks: [project-layer wiring card], [per-window watcher card].

## Review Findings (2026-06-02 16:14)

### Blockers
- [x] `apps/kanban-app/src/commands.rs:2398-2470` — `mcp_subscribe` inserts the window label into `MCP_SUBSCRIBED_WINDOWS` and never removes it, but `handle_board_switch_result` (commands.rs:1533-1537) switches a window's board IN PLACE via `set_window_board(label, new_board)` without recreating the window. After an in-place board switch the label is already in the set, so a re-invocation of `mcp_subscribe` returns early — the window stays subscribed to the OLD board's notification bridge and never binds the new board's, so it silently stops receiving its board's events. Fix: remove the label from `MCP_SUBSCRIBED_WINDOWS` when the window's board assignment changes (in `handle_board_switch_result`/`drop_or_detach_board`) and on window close, so the next `mcp_subscribe` re-binds to the correct board's bridge.

### Warnings
- [x] `apps/kanban-app/src/commands.rs:2296-2298,2455` — `MCP_SUBSCRIBED_WINDOWS` labels are never removed on window close (`on_window_close_requested`/`on_window_destroyed`, main.rs:494-510, clean UIState but not this set). The set grows unbounded across a session and a reused Tauri label would skip re-subscribing. Fix: remove the label on window close (or when the forwarder task sees `RecvError::Closed`).
- [x] `apps/kanban-app/src/state.rs:130-160` — `Drop for BoardHandle` aborts `bridge_task` and async-shuts the `mcp_server`, but drops the new `platform` field inline. `BridgeRuntime::drop` (host.rs:318-324) does a blocking `join()`, and `PluginHost` is the sole `Arc<HostInner>` owner, so closing a board triggers a blocking thread-join inside `close_board` (state.rs:1088-1089) — on a Tokio worker, while holding the `boards` write lock. Same hazard the `mcp_server` shutdown was written to avoid. Fix: take `platform` in `Drop` and drop it on a blocking thread (mirror the `mcp_server` spawned shutdown).
- [x] `apps/kanban-app/src/plugins.rs` (new per-board tests) — tests prove distinct `CommandService` objects + identical baselines but never assert the central guarantee that a plugin in board A's registry is NOT visible in board B's (registry isolation, not object identity); `closing_a_board_drops_its_host_without_affecting_the_other` also can't catch the Drop worker-stall (a completing join still passes). With `project_root` deferred to the next card there's no project plugin to load yet, so this is a coverage gap to close when project plugins land — add an isolation test then.

### Nits
- [x] `apps/kanban-app/src/plugins.rs:520,534` — `BUILTIN_COMMAND_ID = "task.move"` duplicates `BUILTIN_COMMAND_BASELINE[0]`; derive it or document the overlap.
- [x] `apps/kanban-app/src/commands.rs:1392-1399` — boxed-future block indented one level too deep; `cargo fmt` fixes it.

## Review Findings (re-review 2026-06-02 19:40)

Re-review of the fix delta surfaced three nits relating to this card. All fixed.

### Nits
- [x] `apps/kanban-app/src/confine.rs:104` — `run_confined`'s `recv().expect(...)` turned a confined-job panic into a generic "no result" caller panic, discarding the original payload/backtrace. FIXED: wrapped the job in `std::panic::catch_unwind(AssertUnwindSafe(…))`, send a `std::thread::Result<T>` over the channel, and `std::panic::resume_unwind(payload)` on the caller side so the original panic message/backtrace surfaces. The no-hang guarantee is preserved — a still-dropped channel (job vanished without panicking into it) `panic!`s loudly instead of blocking. `AssertUnwindSafe` is sound because the payload is re-raised, not observed.
- [x] `apps/kanban-app/src/commands.rs:2515-2519` — `bind_window_forwarder` resolves `(key, bridge)` before taking the install lock, then re-locks to insert (last-resolve-wins). FIXED: added a comment noting this benign TOCTOU is acceptable because binds for one label are serialized by the board-switch/open path, so the resolve and the install observe the same board assignment.
- [x] `apps/kanban-app/src/commands.rs:2530-2535` — the forwarder task is spawned before its map entry is inserted; if the pump hits `RecvError::Closed` before insertion, its generation-guarded self-evict finds no matching entry and no-ops, leaving the dead entry until the next bind/unbind. FIXED: added a comment noting this is harmless and self-healing — the lingering entry is replaced or removed by the next bind/unbind for the label; no leak across binds.