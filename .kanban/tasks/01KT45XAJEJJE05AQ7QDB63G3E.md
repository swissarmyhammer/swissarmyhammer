---
assignees:
- claude-code
depends_on:
- 01KT45WX7DR10FVVZHQE0QT3JT
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe380
project: plugin-arch
title: Wire the project plugin layer per board (board_dir/.kanban/plugins) + un-ignore it in .kanban gitignore
---
Wire the project layer the platform already supports but the kanban app passes `None` for today.

## Work
- For each per-window host, pass `Some(<board_dir>/.kanban/plugins parent — i.e. board_dir as the project root)` as `PluginHost::new`'s `project_root` (3rd arg; currently `None` at `plugins.rs:142-148`). Discovery already stacks project ▸ user ▸ builtin via `VirtualFileSystem` scoped to `PLUGINS_SUBDIR` (`discovery.rs`), so the layer "just works" once the root is supplied. Project plugins live at `<board_dir>/.kanban/plugins/<plugin-id>/index.ts`.
- The project layer uses the embedder's KanbanConfig namespace: `DIR_NAME=".kanban"`. Confirm the resolved root joins `.kanban/plugins/` correctly per board.
- **Un-ignore project plugins from git.** `KanbanConfig::GITIGNORE_CONTENT` (`crates/swissarmyhammer-directory/src/config.rs:218`) currently ignores everything in `.kanban` except `.gitignore` (`*` then `!.gitignore`). Project plugins are meant to be checked in (repo-shared), so add `!plugins/` and `!plugins/**` exceptions — otherwise the whole point of a repo-checked-in layer is defeated.

## Acceptance
- Dropping `<board_dir>/.kanban/plugins/<id>/index.ts` makes that plugin load in that board's window only (verified against a real board), stacked over user+builtin.
- A project plugin with the same id as a user/builtin one shadows it (project wins) in that window.
- `.kanban/plugins/` is NOT gitignored; the rest of `.kanban` runtime data still is.

Depends on: [per-window PluginHost card].