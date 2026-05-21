# Plan 1 — Store Service (foundational)

**Kanban project:** `store-service` · **Tier 0** · **Depends on:** nothing
(uses the already-merged `operation_tool!` macro).

The bottom of the command stack. Exposes the one shared `StoreContext` (the
single undo substrate) over MCP. Everything undoable rests here.

## Why it's foundational

There is exactly **one** `Arc<StoreContext>` and one `undo_stack.yaml`
(`apps/kanban-app/src/state.rs:281`). Every `TrackedStore` — entities, views,
perspectives — registers into it, so `undo` reverts the last change across all
of them. `command-service`, `command-backends`, and `command-events` all build
on the `store` server + this shared Arc.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS5F5ZNA0621X8KM2NPERXNV` | Shared StoreContext substrate: single undo stack across all command-backing servers | — | One `StoreContext` Arc shared by kanban/views/store/app; cross-server `store.undo` reverts entity + view + perspective changes on one stack; guard test asserts a single instance. |
| `01KS5F7BR6850RKT67X4CNHPAZ` | `store` MCP server: undo/redo/transaction/history over the shared StoreContext | substrate | `store` exposes `Undo`/`Redo`/`CanUndo`/`CanRedo`/`UndoDepth` (stack-wide) + `History`/`GetItem` (store-scoped, take a `store` param); ambient transaction id replaces the global `current_group` mutex; concurrent txns independent. |

## Key decisions baked in

- Undo/redo logic already exists in `swissarmyhammer-store` (`context.rs`,
  `stack.rs`, `changelog.rs` with forward/reverse patches). This plan is the
  **MCP face + the txn mechanism**, not a new undo algorithm.
- Stack-wide ops (undo/redo) take **no** `store` param; store-scoped ops
  (history/get) **require** one — there are many stores.
- **Ambient transaction id**: carried in `RequestContext::extensions`, stamped
  onto each undo entry's `group_id`. Set by the Command service around
  `execute` (see plan 2). Generic — any store honors it; concurrency-safe.

## Cross-check

`kanban list tasks --filter '$store-service'` → expect exactly these 2 tasks.
