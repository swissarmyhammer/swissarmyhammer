# Plan 1 — Store Service (foundational)

**Kanban project:** `store-service` · **Tier 0** · **Depends on:** nothing
(uses the already-merged `operation_tool!` macro).

The bottom of the command stack. Exposes the one shared `StoreContext` (the
single undo substrate) over MCP. Everything undoable rests here.

## Why it's foundational

There is exactly **one** `Arc<StoreContext>` and one `undo_stack.yaml`
(built in `BoardHandle::open` at `apps/kanban-app/src/state.rs:~323`). Every
`TrackedStore` — entity stores (task/column/tag/project/actor) AND view +
perspective stores — registers into it via `Arc::clone(&store_context)`, so
`undo` reverts the last change across all of them in one LIFO stack.
`command-service`, `command-backends`, and `command-events` all build on the
`store` server + this shared Arc.

The substrate **already exists in production**. The two cards here are (1) a
documentation + regression guard pinning that invariant in place, and (2) the
new `store` MCP face that lets callers reach undo/redo/history through the
plugin platform. They are **independent**: card 2 does not block on card 1.

## Tasks

Both cards are standalone — neither depends on the other, and neither depends
on the `kanban` / `views` / `entity` MCP servers that don't exist yet. The
cross-server end-to-end test of `store.undo` reverting entity + view +
perspective edits *through* the MCP face is deliberately deferred to
`command-events` / `command-cutover`, where those servers exist.

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS5F5ZNA0621X8KM2NPERXNV` | Shared StoreContext substrate: single undo stack across all command-backing servers | — | Doc comment + guard test pin the production invariant: one `Arc<StoreContext>` built in `BoardHandle::open`, every `TrackedStore` (entity/view/perspective) registers into it; `Arc::ptr_eq` guard test fails the moment a second `StoreContext` is constructed. No code change to the substrate itself — it is already correct. |
| `01KS5F7BR6850RKT67X4CNHPAZ` | `store` MCP server: undo/redo/transaction/history over the shared StoreContext | — | `store` exposes `Undo`/`Redo`/`CanUndo`/`CanRedo`/`UndoDepth` (stack-wide) + `BeginTransaction`/`EndTransaction` + `History`/`GetItem` (store-scoped, take a `store` param). NOTE `History`/`GetItem` need NET-NEW `ErasedStore`/`StoreContext` accessors (no per-item bytes/changelog reader exists today). Ambient transaction id replaces the global `current_group` mutex internals; concurrent txns independent. Tested in isolation in `swissarmyhammer-store` — no other MCP server required. |

## Key decisions baked in

- Undo/redo logic already exists in `swissarmyhammer-store` (`context.rs`,
  `stack.rs`, `changelog.rs` with forward/reverse patches). This plan is the
  **MCP face + the txn mechanism**, not a new undo algorithm.
- Stack-wide ops (undo/redo) take **no** `store` param; store-scoped ops
  (history/get) **require** one — there are many stores.
- **Ambient transaction id**: carried in `RequestContext::extensions`, stamped
  onto each undo entry's `group_id`. Exposed publicly here as
  `BeginTransaction`/`EndTransaction`; the Command service later sets it
  automatically around `execute` (plan 2 / `01KS613VPH2G4ZWKZPGW9ZCJAA`).
  Generic — any store honors it; concurrency-safe.

## Cross-check

`kanban list tasks --filter '$store-service'` → expect exactly these 2 tasks.
