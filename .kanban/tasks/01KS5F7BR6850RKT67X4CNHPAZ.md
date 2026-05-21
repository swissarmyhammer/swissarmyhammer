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

The undo/redo/grouping ops wrap existing `StoreContext` APIs (verified: `undo()`, `redo()`, `can_undo()`, `can_redo()`, `undo_depth()`, `begin_undo_group()`/`end_undo_group()`, `push()`, `flush_all()`). NOTE — the per-item read ops are NOT a thin face over existing code: `StoreContext`/`ErasedStore` (`erased.rs`) currently expose only `root`/`store_name`/`flush_changes`/`has_entry`/`undo_erased`/`redo_erased` — there is **no** accessor to read an item's current bytes or its changelog by `(store, item_id)`. `Changelog::read_all`/`find_entry` (`changelog.rs`) exist but only per-store, not reachable through the context. So `History` and `GetItem` require **new `ErasedStore` trait methods + new `StoreContext` accessors** — net-new work, not just an MCP wrapper.

Files:
- `crates/swissarmyhammer-store/src/erased.rs` — add `get_item_bytes(item_id)` and `read_changelog(item_id)` (or equivalent) to the `ErasedStore` trait; implement for the concrete store(s)
- `crates/swissarmyhammer-store/src/context.rs` — add `StoreContext` accessors that dispatch by `store` name to the above
- `crates/swissarmyhammer-store/src/server.rs` (or a thin `swissarmyhammer-store-mcp` crate) — `StoreServer` holding the shared `Arc<StoreContext>`
- `operations.rs` — `#[operation]` structs:
  - **stack-wide (no `store` param)**: `Undo`, `Redo`, `CanUndo`, `CanRedo`, `UndoDepth` — operate on the one unified stack; revert/replay whatever store(s) the target entry/group touched. Return `UndoOutcome { items: [(store, item)…] }`.
  - **transaction grouping**: see the grouping mechanism below — exposed so a logical command's multi-store writes undo atomically.
  - **store-scoped (`store` param required)**: `History { store, item_id }` (per-item changelog — needs the new accessor), `GetItem { store, item_id }` (read current bytes — needs the new accessor), optionally `ListStores`.
- bootstrap — `host.expose_rust_module("store", StoreServer::new(shared_store_ctx.clone()))`

### Transaction grouping (generic, cross-store)

A single command often writes several items across several stores (e.g. `column.reorder` → N columns; a paste → a task + its tags) and they must undo as **one** step. Today this uses a global `current_group` single-slot `Mutex<Option<UndoEntryId>>` on `StoreContext` (`context.rs`) — single-group, racy under concurrent dispatch, kanban-unaware of views/perspectives. The existing `begin_undo_group`/`end_undo_group` are the entry points; this task **replaces their mutex internals** with an **ambient transaction id** carried in the call context (it does NOT add a second, parallel grouping mechanism):

- The Command service generates one transaction id per `execute` and stamps it into `RequestContext::extensions` (same channel as `CallerId`).
- The dispatcher propagates it onto every downstream `tools/call` the `execute` callback makes — to `kanban`, `views`, any store-backed server.
- Each store's write path reads the ambient transaction id and passes it as the `group_id` to `StoreContext::push`. Entries sharing the id are one undo group regardless of which store/server produced them.
- No global mutable group state; concurrency-safe; generic — any store that pushes honors the ambient id if present, knows nothing about commands.

`store` exposes the transaction lifecycle for non-command callers too (`BeginTransaction` → id, `EndTransaction`, replacing the `begin/end_undo_group` mutex API), but the common path is the Command service bracketing `execute` automatically.

### Relationship to other servers

- `kanban` / `views` / `entity` **write** into the shared `StoreContext` (their writes push undo entries, stamped with the ambient txn id).
- `store` provides **visibility and control** over the resulting stack (undo/redo/history). No server calls another server's MCP — they share the `Arc<StoreContext>`.

## Acceptance Criteria
- [ ] `store` registered as an in-process server over the shared `Arc<StoreContext>`
- [ ] `Undo`/`Redo`/`CanUndo`/`CanRedo`/`UndoDepth` operate on the one unified stack and revert across stores
- [ ] New `ErasedStore`/`StoreContext` accessors for current-bytes + per-item changelog exist; `History`/`GetItem` use them and require a `store` param; unknown store → structured error
- [ ] Ambient transaction id replaces the global `current_group` (its mutex internals, not a parallel system); a command's multi-store writes undo as one group; concurrent transactions don't interfere
- [ ] `_meta` operations tree complete

## Tests
- [ ] `crates/swissarmyhammer-store/tests/integration/store_server_e2e.rs` — undo/redo round-trips over the shared ctx; `History { store: "task", item_id }` returns the item's changelog (via the new accessor); `GetItem` returns current bytes
- [ ] `crates/swissarmyhammer-store/tests/integration/txn_grouping_e2e.rs` — open a transaction id; make writes to two different stores under it; single `Undo` reverts both; a second concurrent transaction id stays independent
- [ ] `_meta` snapshot
- [ ] `cargo test -p swissarmyhammer-store` passes

## Workflow
- Use `/tdd` — write the cross-store transaction-grouping test first; it pins the generic grouping contract.

Depends on the shared-substrate task and the operation-struct foundation. Prerequisite for: the cache-reconciliation task, the app-shell-commands plugin (app.undo/redo → `store`), and the Command-service execute-bracketing.