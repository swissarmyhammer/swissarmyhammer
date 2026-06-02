---
assignees:
- claude-code
position_column: todo
position_ordinal: ab80
project: plugin-arch
title: Per-isolate CWD for per-board plugin hosts (Deno.cwd → board dir; no process-CWD reliance)
---
DISCOVERED during review of the per-window host work (01KT45WX). User decision: keep per-board PluginHosts in ONE process, but give each board's V8 isolate its OWN cwd. Rationale: an MCP server / plugin must resolve paths relative to ITS board, and process CWD is global — you can't have a per-board process CWD in a single process.

## Problem (verified)
- `std::env::current_dir()` is process-global and is used for the global host's `tool_working_dir` (apps/kanban-app/src/state.rs:1220) and board discovery (`auto_open_board`).
- The deno_core runtime has NO per-isolate cwd (grep of `crates/swissarmyhammer-plugin/src/runtime/*` finds no cwd handling). The plugin SDK exposes only `op_host_dispatch`; `Deno.cwd()` is therefore either absent or, if provided later, would return the single process CWD shared by every per-board host's isolates.
- With per-board hosts in one process, two boards' plugins would see the SAME cwd → any cwd-relative resolution is wrong for at least one board.
- The `kanban` tool is already safe — `expose_kanban_module` roots it via `ToolContext::with_working_dir(board_dir)` (plugins.rs:328), not process CWD. The gap is everything ELSE (plugin `Deno.cwd()`, and any other exposed tool that reads process CWD).

## Work
- **Per-isolate cwd in the runtime.** `crates/swissarmyhammer-plugin`: add a configured cwd to the runtime/host (e.g. `RuntimeConfig { cwd: PathBuf, … }`), and provide a per-isolate `op_cwd` + a `Deno.cwd()` SDK shim that returns THAT host's configured cwd. Each per-board `PluginHost`/`PluginPlatform` is built with its board dir as the isolate cwd. Confirm deno_core specifics (bare deno_core doesn't ship `Deno.cwd`; this likely means *providing* the op, not overriding one).
- **Thread the board dir through.** `PluginPlatform::build` / `PluginHost::new` (apps/kanban-app/src/plugins.rs, state.rs `build_board_platform`) pass the board dir as the isolate cwd, alongside the existing `tool_working_dir`.
- **Audit exposed tools for process-CWD reliance.** Every tool exposed to a per-board host must root at an explicit board working_dir, never `std::env::current_dir()`. kanban is fine; check any other module exposed via `install_app_command_services` / `expose_*`. Fix or document each.
- The global fallback host (boardless) can keep the process cwd or a temp dir — it serves no board.

## Acceptance
- Real-pipeline test: two per-board hosts built for two different board dirs; a probe plugin in each observes `Deno.cwd()` == its OWN board dir (they differ). 
- No exposed tool resolves a board path via `std::env::current_dir()`; all use explicit board working_dir.
- Existing tests stay green.

## Relationship
- Builds on the per-window/per-board host card (01KT45WX). Interacts with the project-layer card (01KT45XA) — project plugins discovered from `<board_dir>/.kanban/plugins/` should also run with cwd == board dir.