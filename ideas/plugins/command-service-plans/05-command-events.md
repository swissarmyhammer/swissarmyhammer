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
| `01KS5G3AKZXDN7K6YR415E0V4K` | MCP notification surface: store-keyed change events for all stored things | op-structs, store | Four planes (store/changed, commands/executed, registry/lifecycle, ephemeral [ui_state + undo_changed]) over MCP with `txn`+`origin`; in-process + external clients receive; bridge sources the existing bus. |
| `01KS5F8THM5EQMKFSF6GFAE55C` | Change propagation: undo/redo emit edit-shaped data events + a stack-state event | store, substrate, notification surface | undo/redo already emit transition-derived events via the reconcile step (redo-of-delete = removed, etc.); keep the reconcile (it IS the convergence), unify its 3-branch dispatch, add `txn`/`origin`; `store/undo_changed` fires on push/undo/redo incl. redo-tail discard. |
| `01KS5G3S1MR6Y77RXPHZP4SZB1` | Frontend: subscribe to MCP change notifications (reuse reducer + txn batching) | notification surface, change propagation | Webview subscribes via MCP, drops Tauri `listen()`; existing reducer reused; N changes sharing a `txn` apply as one atomic re-render; undo controls track `undo_changed`. |

## The four notification planes

1. **`store/changed {store,item,op,changes?,txn,origin}`** — data, for all
   stored things (entities carry field `changes`; views/perspectives reload-item).
2. **`commands/executed {id,ctx,result,txn,origin}`** — semantic action plane
   (Obsidian-style reactive plugins). Emitted by the engine's `execute` (plan 2).
3. **Registry/lifecycle** — `commands/changed`, `tools/list_changed`, board/plugin lifecycle.
4. **Ephemeral** — `ui_state/changed`, `store/undo_changed`.

## Key decisions baked in

- **Undo == edit downstream — already true today, don't rebuild it.** Undo and an
  external file-change converge on the *same* cache calls. For entities, both the
  file watcher (`watcher.rs:210`) and undo's reconcile (`app_commands.rs::reconcile_post_undo_caches:284`
  → `EntityContext::sync_entity_cache_from_disk` — the `UndoCmd`/`RedoCmd` in
  `undo_commands.rs` merely *call into* this reconcile)
  funnel into `cache.refresh_from_disk` / `cache.evict`, which derive the
  field diff (`cache.rs::diff`) and emit the identical `EntityChanged` /
  `EntityDeleted`. `reconcile_post_undo_caches` already covers all three store
  categories the same way: entities, perspectives (`reload_from_disk` →
  `PerspectiveChanged`), views (`reload_from_disk` → `ViewEvent`). The "emit the
  same events as a forward edit, derived from the byte transition" property is
  the *current behavior*, validated by `context.rs:4078`/`:4115`.
- **The reconcile step IS the convergence — keep it, don't delete it.**
  `StoreContext::undo`/`redo` only rewrite disk and return `UndoOutcome`; they
  **cannot** emit entity/view/perspective events (store has no dependency on
  those crates and no event sender — Cargo.toml is diffy/serde/tokio only).
  Emission must stay one layer up, in the reconcile. The work here is to
  **unify** `reconcile_post_undo_caches`'s three hand-written branches into one
  uniform reconcile keyed on `outcome.items` + store category — not to remove
  the trigger (removing it would leave nobody to emit). Idempotency note: the
  watcher will also observe undo's disk writes, but `refresh_from_disk`'s hash
  gate makes that second pass a silent no-op, so there's no double-fire.
- **`txn` correlation + `origin` provenance on every notification** — the genuinely
  net-new field-level work: none of `EntityEvent` / `PerspectiveChanged` /
  `ViewEvent` carry `txn`/`origin` today (`events.rs:31`). Stamp them so a
  command's N changes form one undo group + one atomic UI batch, and so
  `origin: undo|redo|watcher|user|agent` enables attribution / echo-suppression.
- **Transport swap, not a path rebuild.** Today these bus events reach the
  webview via the Tauri bridge (`app.emit` as `entity-field-changed` /
  `entity-removed` / view events). The plan moves the bus→client hop to MCP
  `store/changed`; the reconcile and the bus emission are unchanged.
- **Stack-state is the only truly new event.** `store/undo_changed`
  (`can_undo`/`can_redo`/labels) has no equivalent today — the UIState flags
  exist but no per-mutation notification does. It must fire on plain `push` too
  (a normal edit after an undo truncates the redo tail), which no data event
  carries.
- **Stored things ⊋ entities** — schema is store-keyed; "entity" is one category.
- Frontend: **reuse the reducer, swap the source, add txn batching** — not a
  reload rebuild.

## Cross-check

`kanban list tasks --filter '$command-events'` → expect exactly these 3 tasks.
