---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb680
project: store-service
title: '`store` MCP server: undo/redo/transaction/history over the shared StoreContext'
---
## What

Expose the shared `StoreContext` (the single undo substrate — already wired in production at `apps/kanban-app/src/state.rs`) as an in-process MCP server named `store`, registered via `host.expose_rust_module(...)` on the existing `swissarmyhammer-plugin` host. This is the generic, store-layer surface for the cross-cutting concerns that aren't entity-type-specific: undo, redo, transaction grouping, and per-item history.

Because the substrate owns **multiple** stores (task, column, tag, project, actor, view, perspective), store-scoped operations take a **`store` parameter**; the stack-wide operations (undo/redo) do not, by design.

The undo/redo/grouping ops wrap existing `StoreContext` APIs that ship today: `undo()`, `redo()`, `can_undo()`, `can_redo()`, `undo_depth()`, `begin_undo_group()`/`end_undo_group()`, `push()`, `flush_all()` (verified in `crates/swissarmyhammer-store/src/context.rs`).

The per-item READ ops are **net-new work**, not a thin face over existing code. `StoreContext`/`ErasedStore` (`erased.rs`) today expose only `root` / `store_name` / `flush_changes` / `has_entry` / `undo_erased` / `redo_erased` — **no** accessor reads an item's current bytes or its changelog by `(store, item_id)`. `Changelog::read_all` / `find_entry` exist but only per-store, unreachable through the context. So `History` and `GetItem` require **new `ErasedStore` trait methods + new `StoreContext` accessors**.

This card is standalone — Tier 0 by design. The substrate already exists in production (the guard-test card `01KS5F5ZNA0621X8KM2NPERXNV` documents the invariant; it does not *create* the substrate, so this card does not block on it). The only external dependency is the `operation_tool!` macro, which is already merged in `swissarmyhammer-operations-macros`.

Files:
- `crates/swissarmyhammer-store/src/erased.rs` — add `get_item_bytes(item_id)` and `read_changelog(item_id)` (or equivalent) to the `ErasedStore` trait; implement for the concrete store(s).
- `crates/swissarmyhammer-store/src/context.rs` — add `StoreContext` accessors that dispatch by `store` name to the above.
- `crates/swissarmyhammer-store/src/server.rs` (new module) — `StoreServer` holding the shared `Arc<StoreContext>` and implementing the plugin platform's `McpServer` trait.
- `crates/swissarmyhammer-store/src/operations.rs` (new module) — `#[operation]` structs via the merged `operation_tool!` macro:
  - **stack-wide (no `store` param)**: `Undo`, `Redo`, `CanUndo`, `CanRedo`, `UndoDepth` — operate on the one unified stack; revert/replay whatever store(s) the target entry/group touched. Return `UndoOutcome { items: [(store, item)…] }`.
  - **transaction grouping**: `BeginTransaction` → id, `EndTransaction` — the public lifecycle for non-command callers. Internally this replaces the global `current_group: Mutex<Option<UndoEntryId>>` slot on `StoreContext` with an ambient transaction id (see below). Existing `begin_undo_group`/`end_undo_group` callers are migrated; no parallel grouping mechanism.
  - **store-scoped (`store` param required)**: `History { store, item_id }` (per-item changelog — needs the new accessor), `GetItem { store, item_id }` (read current bytes — needs the new accessor), optionally `ListStores`.
- bootstrap call sites — `host.expose_rust_module("store", StoreServer::new(shared_store_ctx.clone()))` so a plugin can activate the module under any name with `register("store", { rust: "store" })`. (No bootstrap change in *production* yet — that lives in the cut-over project. For this card, the wiring is exercised through the integration tests.)

### Transaction grouping (generic, cross-store)

A single command often writes several items across several stores (e.g. a column reorder → N columns; a paste → a task + its tags) and they must undo as **one** step. Today this uses a global `current_group` single-slot `Mutex<Option<UndoEntryId>>` on `StoreContext` — single-group, racy under concurrent dispatch. The existing `begin_undo_group` / `end_undo_group` are the entry points; this task **replaces their mutex internals** with an **ambient transaction id** that can be carried in a call's `RequestContext::extensions`:

- A `BeginTransaction` (or the Command service, later) generates a `txn` id and sets it as an ambient value.
- Each store's write path reads the ambient `txn` if present and passes it as `group_id` to `StoreContext::push`. Entries sharing the id are one undo group regardless of which store produced them. Without an ambient `txn`, writes group per the existing legacy rule.
- No global mutable group state; concurrency-safe; generic — any store that pushes honors the ambient id if present; the store knows nothing about commands.

The ambient-`txn` mechanism is **testable in isolation** here: tests call `BeginTransaction` directly, make writes through two different `TrackedStore`s under it, and assert a single `Undo` reverts both. The Command-service-driven case (the txn is set automatically around `execute`) is the follow-up card `01KS613VPH2G4ZWKZPGW9ZCJAA` in `command-service`.

### Relationship to other servers (informational, not a dependency)

- `kanban` / `views` / `entity` MCP servers (future) **write** into the shared `StoreContext` — their writes push undo entries, stamped with the ambient txn id when one is set.
- `store` provides **visibility and control** over the resulting stack (undo / redo / history). No server calls another server's MCP — they share the `Arc<StoreContext>`.

None of those other servers need to exist for THIS card to land — the store ops are exercised directly against the existing `TrackedStore` set through integration tests.

## Acceptance Criteria
- [ ] `store` server type exists and registers via `host.expose_rust_module("store", …)` against a shared `Arc<StoreContext>`
- [ ] `Undo` / `Redo` / `CanUndo` / `CanRedo` / `UndoDepth` operate on the one unified stack and revert across stores when invoked through the MCP face
- [ ] New `ErasedStore` / `StoreContext` accessors for current-bytes + per-item changelog exist; `History` / `GetItem` use them and require a `store` param; unknown store → structured error
- [ ] Ambient transaction id replaces the global `current_group` (its mutex internals, not a parallel system); a `BeginTransaction` → writes through two stores → `EndTransaction` → single `Undo` reverts both; concurrent transactions don't interfere
- [ ] `_meta` operations tree complete (one `store` tool, all ops surfaced under it)
- [ ] No dependency on the `kanban` / `views` / `entity` MCP servers — the integration tests stand the substrate up directly and drive the `store` ops against it

## Tests
- [ ] `crates/swissarmyhammer-store/tests/integration/store_server_e2e.rs` — undo/redo round-trips over the shared ctx; `History { store: "task", item_id }` returns the item's changelog (via the new accessor); `GetItem` returns current bytes
- [ ] `crates/swissarmyhammer-store/tests/integration/txn_grouping_e2e.rs` — open a transaction id; make writes to two different stores under it; single `Undo` reverts both; a second concurrent transaction id stays independent
- [ ] `crates/swissarmyhammer-store/tests/integration/meta_snapshot.rs` — `_meta` operations-tree snapshot for the `store` tool
- [ ] `cargo test -p swissarmyhammer-store` passes

## Workflow
- Use `/tdd` — write the cross-store transaction-grouping test first; it pins the generic grouping contract.

Standalone — no kanban dep, no cross-server e2e. Uses only the existing `swissarmyhammer-store` crate, the existing `swissarmyhammer-plugin` host, and the already-merged `operation_tool!` macro. Prerequisite for: the cache-reconciliation card, the app-shell `app.undo/redo` plugin (which forwards to this `store` server), and the Command-service `execute` transaction-bracketing follow-up (`01KS613VPH2G4ZWKZPGW9ZCJAA`).