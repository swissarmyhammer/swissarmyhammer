---
assignees:
- claude-code
depends_on:
- 01KS5F5ZNA0621X8KM2NPERXNV
position_column: todo
position_ordinal: '9780'
project: command-backends
title: '`views` MCP server (perspective + view state)'
---
## What

Build an in-process MCP server `views` that wraps `PerspectiveContext` + `ViewsContext`, exposing the perspective/view mutations that the `perspective.*` (17) and `view.set` commands depend on. Today these are purely in-process (`ctx.kanban.views()` RwLock, `perspective_context_if_ready()`) with NO MCP surface — this server is net-new and a hard prerequisite for the perspective-commands plugin and the view command.

Files:
- `crates/swissarmyhammer-views/...` already exists in the workspace (`crates/swissarmyhammer-views/`). Prefer adding the MCP server there rather than a new crate, since the state lives there. If `PerspectiveContext`/`ViewsContext` live in `swissarmyhammer-kanban/src/context.rs`, either move them to `swissarmyhammer-views` or expose them from a server in `swissarmyhammer-kanban`. Decide based on where the structs already are — do not duplicate state.
- `operations.rs` — `#[operation]` structs covering:
  - lifecycle: `LoadPerspective`, `SavePerspective`, `DeletePerspective`, `RenamePerspective`, `ListPerspective`
  - filter: `SetFilter`, `FocusFilter`, `ClearFilter`
  - group: `SetGroup`, `ClearGroup`
  - sort: `SetSort`, `ClearSort`, `ToggleSort`
  - nav: `NextPerspective`, `PrevPerspective`, `GotoPerspective`, `SwitchPerspective`
  - view: `SetView`
- `service.rs` — `ViewsServer` holding the perspective + views contexts
- bootstrap — `host.expose_rust_module("views", ViewsServer::new(...))`

1:1 port of today's perspective_commands.rs / view command behavior into rmcp operations. Filter expressions, sort entries, group keys keep their existing shapes (note the `expr-filter` project may change filter representation — coordinate, but for this task preserve current behavior).

Undoable perspective mutations (save/delete/rename/filter/group/sort) write through the unified changelog (single-changelog kernel), so `app.undo` reverts them — this server does NOT implement its own undo.

## Acceptance Criteria
- [ ] `views` registered as an in-process server at bootstrap
- [ ] All 17 perspective operations + `set view` reachable via MCP
- [ ] No duplicate perspective/view state — the server wraps the existing context structs, wherever they live
- [ ] Mutations are captured by the unified changelog so `app.undo` can revert them
- [ ] `_meta` operations tree complete

## Tests
- [ ] `crates/swissarmyhammer-views/tests/integration/views_e2e.rs` — per-sub-domain tests: save→load roundtrip; set filter→assert active filter; set/clear/toggle sort; group/clearGroup; next/prev/goto/switch navigation; set view. Real server, observe persisted perspective state.
- [ ] Undo integration: mutate a perspective (e.g. set filter); `app.undo`; assert the filter reverted — proves the changelog captures view mutations
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-views` passes

## Workflow
- Use `/tdd`

Prerequisite for: perspective-commands plugin, the `view.set` command (in kanban-misc-commands). Depends on the operation-struct foundation + plugin-arch macro work.