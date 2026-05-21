# Plan 5 — Command Events (notifications → UI)

**Kanban project:** `command-events` · **Tier 2** · **Depends on:**
`store-service` (txn + the shared bus), `command-service` (action events), and
`command-backends` (the servers whose state changes).

Event propagation to UI **and** agents over MCP. The premise: all app-facing
APIs are MCP, so the webview and AI agents subscribe to the *same* change
stream. Kept light because the UI's entity/field reducer already works — we
change its *source*, not its logic.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS5G3AKZXDN7K6YR415E0V4K` | MCP notification surface: store-keyed change events for all stored things | op-structs, store | Four planes (store/changed, commands/executed, registry, ui_state) over MCP with `txn`+`origin`; in-process + external clients receive; bridge sources the existing bus. |
| `01KS5F8THM5EQMKFSF6GFAE55C` | Change propagation: undo/redo emit edit-shaped data events + a stack-state event | store, substrate, notification surface | undo/redo emit transition-derived events (redo-of-delete = removed, etc.); `store/undo_changed` fires on push/undo/redo incl. redo-tail discard; `reconcile_post_undo_caches` removed. |
| `01KS5G3S1MR6Y77RXPHZP4SZB1` | Frontend: subscribe to MCP change notifications (reuse reducer + txn batching) | notification surface, change propagation | Webview subscribes via MCP, drops Tauri `listen()`; existing reducer reused; N changes sharing a `txn` apply as one atomic re-render; undo controls track `undo_changed`. |

## The four notification planes

1. **`store/changed {store,item,op,changes?,txn,origin}`** — data, for all
   stored things (entities carry field `changes`; views/perspectives reload-item).
2. **`commands/executed {id,ctx,result,txn,origin}`** — semantic action plane
   (Obsidian-style reactive plugins). Emitted by the engine's `execute` (plan 2).
3. **Registry/lifecycle** — `commands/changed`, `tools/list_changed`, board/plugin lifecycle.
4. **Ephemeral** — `ui_state/changed`, `store/undo_changed`.

## Key decisions baked in

- **Undo == edit downstream**: undo/redo emit the same events as a forward edit,
  derived from the byte transition, so caches + UI react through existing paths.
- **`txn` correlation + `origin` provenance on every notification** — ties a
  command's N changes into one undo group + one atomic UI batch; enables
  attribution / echo-suppression.
- **Stored things ⊋ entities** — schema is store-keyed; "entity" is one category.
- Frontend: **reuse the reducer, swap the source, add txn batching** — not a
  reload rebuild.

## Cross-check

`kanban list tasks --filter '$command-events'` → expect exactly these 3 tasks.
