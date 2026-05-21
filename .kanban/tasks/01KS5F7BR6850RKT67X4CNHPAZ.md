---
assignees:
- claude-code
depends_on:
- 01KS5F5ZNA0621X8KM2NPERXNV
position_column: todo
position_ordinal: 9a80
project: store-service
title: '`store` MCP server: undo/redo/transaction/history over the shared StoreContext'
---
## What

Expose the shared `StoreContext` (the single undo substrate — see the shared-substrate task) as an MCP server named `store`. This is the generic, store-layer surface for the cross-cutting concerns that aren't entity-type-specific: undo, redo, transaction grouping, and per-item history. Because the substrate owns **multiple** stores (task, column, tag, project, actor, view, perspective), store-scoped operations take a **`store` parameter**; the stack-wide operations (undo/redo) do not, by design.

Wraps the existing `swissarmyhammer-store::StoreContext` (verified APIs: `undo()`, `redo()`, `can_undo()`, `can_redo()`, `undo_depth()`, `begin_undo_group()`, `flush_all()`). No new undo algorithm — this is the MCP face of code that already works.

Files:
- `crates/swissarmyhammer-store/src/server.rs` (or a thin `swissarmyhammer-store-mcp` crate) — `StoreServer` holding the shared `Arc<StoreContext>`
- `operations.rs` — `#[operation]` structs:
  - **stack-wide (no `store` param)**: `Undo`, `Redo`, `CanUndo`, `CanRedo`, `UndoDepth` — operate on the one unified stack; revert/replay whatever store(s) the target entry/group touched. Return `UndoOutcome { items: [(store, item)…] }`.
  - **transaction grouping**: see the grouping mechanism below — exposed so a logical command's multi-store writes undo atomically.
  - **store-scoped (`store` param required)**: `History { store, item_id }` (per-item changelog), `GetItem { store, item_id }` (read current bytes), optionally `ListStores`.
- bootstrap — `host.expose_rust_module("store", StoreServer::new(shared_store_ctx.clone()))`

### Transaction grouping (generic, cross-store)

A single command often writes several items across several stores (e.g. `column.reorder` → N columns; a paste → a task + its tags) and they must undo as **one** step. Today this uses a global `current_group` mutex on `StoreContext` — single-group, racy under concurrent dispatch, and kanban-unaware of views/perspectives. Replace it with an **ambient transaction id** carried in the call context:

- The Command service generates one transaction id per `execute` and stamps it into `RequestContext::extensions` (same channel as `CallerId`).
- The dispatcher propagates it onto every downstream `tools/call` the `execute` callback makes — to `kanban`, `views`, any store-backed server.
- Each store's write path reads the ambient transaction id and passes it as the `group_id` to `StoreContext::push`. Entries sharing the id are one undo group regardless of which store/server produced them.
- No global mutable group state; concurrency-safe; generic — any store that pushes honors the ambient id if present, knows nothing about commands.

`store` exposes the transaction lifecycle for non-command callers too (`BeginTransaction` → id, `EndTransaction`), but the common path is the Command service bracketing `execute` automatically.

### Relationship to other servers

- `kanban` / `views` **write** into the shared `StoreContext` (their writes push undo entries, stamped with the ambient txn id).
- `store` provides **visibility and control** over the resulting stack (undo/redo/history). No server calls another server's MCP — they share the `Arc<StoreContext>`.

## Acceptance Criteria
- [ ] `store` registered as an in-process server over the shared `Arc<StoreContext>`
- [ ] `Undo`/`Redo`/`CanUndo`/`CanRedo`/`UndoDepth` operate on the one unified stack and revert across stores
- [ ] Store-scoped ops (`History`, `GetItem`) require and honor a `store` parameter; unknown store → structured error
- [ ] Ambient transaction id replaces the global `current_group`; a command's multi-store writes undo as one group; concurrent transactions don't interfere
- [ ] `_meta` operations tree complete

## Tests
- [ ] `crates/swissarmyhammer-store/tests/integration/store_server_e2e.rs` — undo/redo round-trips over the shared ctx; `History { store: "task", item_id }` returns the item's changelog; `GetItem` returns current bytes
- [ ] `crates/swissarmyhammer-store/tests/integration/txn_grouping_e2e.rs` — open a transaction id; make writes to two different stores under it; single `Undo` reverts both; a second concurrent transaction id stays independent
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-store` passes

## Workflow
- Use `/tdd` — write the cross-store transaction-grouping test first; it pins the generic grouping contract.

Depends on the shared-substrate task and the operation-struct foundation. Prerequisite for: the cache-reconciliation task, the app-shell-commands plugin (app.undo/redo → `store`), and the Command-service execute-bracketing.