---
assignees:
- claude-code
depends_on:
- 01KS5F5ZNA0621X8KM2NPERXNV
position_column: todo
position_ordinal: '9880'
project: entity-service
title: '`entity` MCP server: generic CRUD + archive + clipboard + search'
---
## What

Build the `entity` MCP server — the generic, type-agnostic MCP face over the entity **kernel** (`EntityContext`/`EntityCache`/`EntityTypeStore` in `swissarmyhammer-entity`), plus `PasteMatrix` (`swissarmyhammer-kanban`) and `EntitySearchIndex` (`swissarmyhammer-entity-search`). Today this state is reached via Tauri commands (`get_entity`) and `ctx.require_extension` — no MCP surface. This server exposes it generically.

### Kernel + faces — kanban's OPERATIONS ARE FROZEN

`EntityContext` is the **kernel** (one CRUD implementation), and both faces already sit over it today:
- **`entity`** (this task, NEW) — generic, type-agnostic: get/list/add/update/delete + archive/unarchive + clipboard + search for ANY type. For cross-cutting `entity.*` commands, the frontend's generic reads, and the search UI.
- **`kanban`** (EXISTS) — domain face: `add task`, `add project`, `update column`, `get tag`, `move task`, `next/complete task`, `assign`, `tag/untag`, board lifecycle.

**Hard constraint (per the user): this work does NOT change kanban's operations.** No operation is added to or removed from the `kanban` tool. The `entity` server is purely additive. kanban already reaches `EntityContext` today, so it needs no change to share the kernel. Refactoring kanban's internals to delegate to the kernel more explicitly is **optional and out of scope right now** — implementation may change later; the operation surface may not.

Files:
- `crates/swissarmyhammer-entity/src/server.rs` (or a thin `swissarmyhammer-entity-mcp` crate) — `EntityServer` over `Arc<EntityContext>` + shared `Arc<StoreContext>` + `EntitySearchIndex`
- `operations.rs` — `#[operation]` structs (entity-type param where relevant):
  - **read**: `GetEntity { type, id }`, `ListEntities { type, filter? }` (replaces the `get_entity` Tauri command + board-load)
  - **write**: `AddEntity { type, fields }`, `UpdateField { type, id, field, value }`, `DeleteEntity { type, id }`
  - **archive**: `ArchiveEntity`, `UnarchiveEntity`
  - **clipboard**: `Cut`, `Copy`, `Paste` (wraps `PasteMatrix`; preserves drag-vs-paste)
  - **search**: `Search { query, type? }` (wraps `EntitySearchIndex`)
- `service.rs` — bootstrap `host.expose_rust_module("entity", EntityServer::new(...))`

Writes go through `EntityContext`, which already pushes onto the shared `StoreContext` (undoable) and broadcasts `EntityEvent`s — so undo + the notification surface work for free. Share the SAME `Arc<StoreContext>` as `kanban`/`views`/`store`.

## Acceptance Criteria
- [ ] `entity` registered as an in-process server over `EntityContext` + shared `StoreContext` + `EntitySearchIndex`
- [ ] Generic read/write/archive/clipboard/search work for any entity type; writes undoable + emit entity events
- [ ] **kanban's operation surface is byte-for-byte unchanged** — no op added or removed; a snapshot guard test asserts kanban's `tools/list` + `_meta` operations tree is identical before and after this work
- [ ] `entity` and `kanban` resolve through the one `EntityContext` kernel (no duplicate CRUD)
- [ ] `_meta` operations tree complete

## Tests
- [ ] `crates/swissarmyhammer-entity/tests/integration/entity_server_e2e.rs` — add → get → update_field → delete across two types; archive/unarchive; clipboard copy→paste duplicates; `Search` finds by text
- [ ] **kanban-surface-frozen guard**: snapshot the `kanban` tool's `_meta` operations tree; assert this work leaves it unchanged
- [ ] Parity: `kanban add task` and `entity AddEntity{type:task}` produce the same on-disk result (both via the kernel)
- [ ] Undo: update_field via `entity`; `store.undo`; assert reverted
- [ ] `cargo test -p swissarmyhammer-entity` passes

## Workflow
- Use `/tdd` — write the kanban-surface-frozen guard + the CRUD/search test first.

Prerequisite for: entity-commands plugin, the frontend's generic reads + search UI. Depends on store-service substrate.