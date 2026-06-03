---
assignees:
- claude-code
depends_on:
- 01KT45WX7DR10FVVZHQE0QT3JT
- 01KT45XAJEJJE05AQ7QDB63G3E
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe480
project: plugin-arch
title: Per-window hot-reload watcher includes the board's .kanban/plugins dir
---
Extend hot reload to the per-board project layer.

## Current state
The watcher watches only the user-layer `plugins/` dir (`plugins.rs:249-261` `start_watcher`, rooted at `user_root.join("plugins")`). Builtin is read-only (not watched). There is no project-layer watching because there's no project layer wired yet.

## Work
- For each per-window host, start a hot-reload watcher covering BOTH the shared user `plugins/` dir AND that window's board `<board_dir>/.kanban/plugins/`. An edit/add/remove under the board's project plugins must reload/load/unload that plugin in THAT window's host only.
- Tear the watcher down when the window's host is torn down (board close/switch) so closing a board never leaks a watcher (mirror the existing per-board MCP server teardown).
- Multiple windows on the same user dir each watch it — dedupe if cheap, otherwise acceptable.

## Acceptance
- Editing a project plugin's `index.ts` under an open board's `.kanban/plugins/` hot-reloads it in that board's window (observe new behavior in the same PluginHost), and does NOT touch other windows.
- Adding/removing a project plugin dir loads/unloads it live in that window.
- Closing the board window stops its watcher (no leak).

Depends on: [per-window PluginHost card], [project-layer wiring card].