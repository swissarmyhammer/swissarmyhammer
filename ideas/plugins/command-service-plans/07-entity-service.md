# Plan 7 — Entity Service (generic data layer)

**Kanban project:** `entity-service` · **Tier 1 (foundational data layer)** ·
**Depends on:** `store-service` (writes are undoable via the shared
`StoreContext`); the merged `operation_tool!` macro.

The `entity` MCP server: the **generic, type-agnostic** entity capability the UI
and the cross-cutting `entity.*` commands need. This is the answer to "you need
an entity service" — generic entity capability is *not* the kanban service.

## Kernel + two faces (kanban keeps everything)

`EntityContext` is the entity **kernel** — one CRUD implementation. Two MCP
faces sit over it:

- **`entity`** (this service) — the **generic** face: get/list/add/update/delete,
  archive/unarchive, clipboard cut/copy/paste, and **search**, for **any** type.
  Backed by `EntityContext`/`EntityCache`/`EntityTypeStore` + `PasteMatrix` +
  `EntitySearchIndex`.
- **`kanban`** — the **domain** face. It **keeps its full surface** (`add task`,
  `add project`, `update column`, `get tag`, `move task`, `next/complete`,
  `assign`, `tag/untag`, board lifecycle) — agents and the CLI rely on these.
  Its generic CRUD **passes through to the kernel** internally, so there's one
  implementation and kanban loses nothing.

So `entity` is **additive** — a new generic face over the shared kernel, not a
removal from kanban. (Search is an entity capability — it lives here, not as a
separate `search` server.) The kanban service needs **no** ops removed; the only
kanban changes elsewhere are shared-`StoreContext` wiring + feeding the
notification bus.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS5EAD57PCBFJGMVB74FF4MK` | `entity` MCP server: generic CRUD + archive + clipboard + search | store-service substrate | `entity` over EntityContext + shared StoreContext + EntitySearchIndex; generic get/list/add/update/delete + archive/unarchive + cut/copy/paste + search for any type; writes undoable + emit entity events; replaces the `get_entity` Tauri command. |

## Consumed by

- **entity-commands plugin** (`entity.*`) → all 8 commands route here (they're
  cross-cutting / `from: target`, so the generic face is the right fit).
- **frontend** → generic reads (replacing `get_entity` Tauri command) + the
  search UI (`Search`). `app.search` opens the search palette via `ui_state`;
  the query itself calls `entity` `Search`.

(`tag.update` and other typed domain commands route to the **kanban** face, which
delegates to this same kernel — not to `entity` directly.)

## Key decisions baked in

- Generic entity ops live here, not on `kanban` — clean capability boundary.
- **Search is an entity capability** (one `Search` op on `entity`), per the
  user's call — no standalone search server.
- Wraps existing in-process state (no duplication); writes go through
  `EntityContext` which already pushes onto the shared undo stack and broadcasts
  `EntityEvent`s, so undo + the notification surface work for free.

## Cross-check

`kanban list tasks --filter '$entity-service'` → expect exactly this 1 task.
