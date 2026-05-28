---
assignees:
- claude-code
depends_on:
- 01KS5F5ZNA0621X8KM2NPERXNV
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffba80
project: command-backends
title: '`views` MCP server (perspective + view state)'
---
## What

Build an in-process MCP server `views` that wraps `PerspectiveContext` + `ViewsContext`, exposing the perspective/view mutations that the `perspective.*` (17) and `view.set` commands depend on. Today these are purely in-process (`ctx.kanban.views()` RwLock, `perspective_context_if_ready()`) with NO MCP surface — this server is net-new and a hard prerequisite for the perspective-commands plugin and the view command.

CRATE LAYOUT (verified — the state spans TWO crates):
- `PerspectiveStore` + `PerspectiveContext` live in `crates/swissarmyhammer-perspectives/` (`store.rs`, `context.rs`).
- `ViewStore` + `ViewsContext` live in `crates/swissarmyhammer-views/`.
Both already `impl TrackedStore` and are already registered on the shared `StoreContext` with undo wiring (`apps/kanban-app/src/state.rs:153-181`); `PerspectiveContext::set_store_context` exists.

Pick ONE host crate for the `views` server and depend on the other — do NOT move structs between crates and do NOT duplicate state. Recommended: host the server in `swissarmyhammer-views` and add a dependency on `swissarmyhammer-perspectives` (or, if that dep direction is awkward, host it in `swissarmyhammer-kanban` which already wires both). State the chosen host in the implementation; the constraint is one server, both contexts, zero duplication.

Files:
- `<host crate>/src/server.rs` — `ViewsServer` holding `Arc<RwLock<PerspectiveContext>>` (from swissarmyhammer-perspectives) + the `ViewsContext` (from swissarmyhammer-views)
- `operations.rs` — `#[operation]` structs covering:
  - lifecycle: `LoadPerspective`, `SavePerspective`, `DeletePerspective`, `RenamePerspective`, `ListPerspective`
  - filter: `SetFilter`, `FocusFilter`, `ClearFilter`
  - group: `SetGroup`, `ClearGroup`
  - sort: `SetSort`, `ClearSort`, `ToggleSort`
  - nav: `NextPerspective`, `PrevPerspective`, `GotoPerspective`, `SwitchPerspective`
  - view: `SetView`
- bootstrap — `host.expose_rust_module("views", ViewsServer::new(...))`

1:1 port of today's perspective_commands.rs / view command behavior into rmcp operations. Filter expressions, sort entries, group keys keep their existing shapes (note the `expr-filter` project may change filter representation — coordinate, but for this task preserve current behavior).

Undoable perspective mutations (save/delete/rename/filter/group/sort) write through the unified changelog (single-changelog kernel), so `store.undo` reverts them — this server does NOT implement its own undo.

## Acceptance Criteria
- [ ] `views` registered as an in-process server at bootstrap, hosting BOTH the perspective context (swissarmyhammer-perspectives) and the views context (swissarmyhammer-views) from one chosen host crate
- [ ] All 17 perspective operations + `set view` reachable via MCP
- [ ] No duplicate perspective/view state — the server wraps the existing context structs in their existing crates
- [ ] Mutations are captured by the unified changelog so `store.undo` can revert them
- [ ] `_meta` operations tree complete

## Tests
- [ ] `<host crate>/tests/integration/views_e2e.rs` — per-sub-domain tests: save→load roundtrip; set filter→assert active filter; set/clear/toggle sort; group/clearGroup; next/prev/goto/switch navigation; set view. Real server, observe persisted perspective state.
- [ ] Undo integration: mutate a perspective (e.g. set filter); `store.undo`; assert the filter reverted — proves the changelog captures view mutations
- [ ] `_meta` snapshot
- [ ] `cargo test -p <host crate>` passes

## Workflow
- Use `/tdd`

Prerequisite for: perspective-commands plugin, the `view.set` command (in kanban-misc-commands). Depends on the operation-struct foundation + plugin-arch macro work + the shared StoreContext substrate.