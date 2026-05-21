# Command Service — Plan Index

Review artifact mirroring the kanban board. The board is the source of truth;
these docs are a frozen snapshot for review and cross-check. Each plan doc lists
its kanban task IDs so you can open the board side-by-side.

Goal: replace the YAML command architecture (`swissarmyhammer-commands` +
`swissarmyhammer-kanban/builtin/commands/`) with an MCP Command Service whose
commands are TypeScript plugins, on the swissarmyhammer plugin platform. Source
design: [`../command-service.md`](../command-service.md).

## The seven plans (build order)

```
Tier 0 (foundational, parallel)
  store-service ────────────┐
  command-service (engine) ─┤
                            │
Tier 1 (data + domain servers)
  entity-service ◄──────────┤   (generic entity CRUD/clipboard/search; needs store-service)
  command-backends ◄────────┘   (views needs store-service; window/app/ui_state independent)
                            │
Tier 2 (parallel)           │
  builtin-commands ◄────────┤   (needs command-service + entity-service + command-backends)
  command-events ◄──────────┘   (needs store-service + command-service + entity + backends)
                            │
Tier 3 (terminal)           │
  command-cutover ◄─────────┘   (needs builtin-commands + command-events)
```

| Plan | Project id | Tasks | Purpose |
| ---- | ---------- | ----: | ------- |
| [Store Service](./01-store-service.md) | `store-service` | 2 | Shared `StoreContext` substrate + `store` MCP (undo/redo/txn/history) |
| [Command Service (engine)](./02-command-service-engine.md) | `command-service` | 7 | The Command MCP engine: verbs, registry/override-stack, callbacks, SDK helpers |
| [Entity Service](./07-entity-service.md) | `entity-service` | 1 | Generic `entity` MCP: type-agnostic CRUD + archive + clipboard + **search** |
| [Command Backends](./03-command-backends.md) | `command-backends` | 4 | Domain servers: views, ui_state, window, app |
| [Builtin Commands](./04-builtin-commands.md) | `builtin-commands` | 10 | Catalog + 7 command plugins + frontend command dispatch |
| [Command Events](./05-command-events.md) | `command-events` | 3 | MCP notification surface, undo/redo propagation, frontend subscription |
| [Command Cut-over](./06-command-cutover.md) | `command-cutover` | 2 | Tauri `invoke()` migration + delete the old crate/YAML |

**Service taxonomy (kernel + two faces):** `EntityContext` is the entity kernel;
both faces sit over it and kanban keeps its full surface.
- **`entity`** — the *generic* face: get/list/add/update/delete, archive/unarchive,
  clipboard cut/copy/paste, and **search**, for any type (search is an entity
  capability, not a separate server).
- **`kanban`** — the *domain* face: keeps ALL its ops (`add task`/`add project`/
  `update column`/…, `move task`, `next/complete`, `assign`, `tag/untag`, board
  lifecycle); generic CRUD delegates to the kernel. **Nothing is removed from kanban.**

Related work in **other projects**:
- `plugin-arch` · `01KS371KNY4YARZ67KWVSXPDFP` — idempotent `ServerRegistry::register`
  (so multiple plugins can `ensureServices` the same server).
- `plugin-arch` — `operation_tool!` macro + `generate_operations_meta` **already merged**.
- `spatial-nav` · `01KS5MYQRB1E5HQ9JJ6TC7Z59S` — a `focus` MCP server over
  `SpatialRegistry`/`SpatialState`; `ui.setFocus` + the `spatial_*` frontend calls
  route to it. Owned by spatial-nav; the command plans consume it.

## Cross-cutting concepts (read once, referenced everywhere)

- **Single undo substrate.** One `Arc<StoreContext>` (already constructed at
  `apps/kanban-app/src/state.rs:281`) owns every `TrackedStore` — entities
  (task/tag/column/project/actor), views, perspectives — and one
  `undo_stack.yaml`. `store.undo` reverts the last entry/group across *all* of
  them. The `kanban`, `views`, and `store` servers share this one Arc.
- **Stored things ⊋ entities.** Three `TrackedStore` categories: entities (rich
  field-level events), views, perspectives (coarse reload-item events today).
  Plus stored-but-not-tracked: `UIState` (own JSON, not undoable). The event
  schema is store-keyed, not entity-keyed.
- **`undoable` is declarative.** Undo is recorded by the store layer on every
  tracked write, not gated by the YAML `undoable` flag.
- **Transaction correlation (`txn`) + provenance (`origin`).** The Command
  service opens an ambient `txn` around each `execute` (propagated via
  `RequestContext::extensions`). Every store write stamps its undo `group_id`
  AND its emitted change events with that `txn`, so a command's N changes form
  one undo group and one atomic UI batch. `origin` = user / agent:id / undo /
  redo / watcher.
- **Four notification planes** (all over MCP, UI and agents subscribe alike):
  1. `store/changed {store,item,op,changes?,txn,origin}` — data
  2. `commands/executed {id,ctx,result,txn,origin}` — semantic action
  3. `commands/changed` / `tools/list_changed` / lifecycle — registry
  4. `ui_state/changed`, `store/undo_changed` — ephemeral / stack state
- **Undo == edit, downstream.** Undo/redo emit the same events as a forward
  edit (derived from the byte transition), so caches + UI react through the
  paths they already use. The existing entity/field reducer is reused; only its
  source changes (Tauri → MCP) plus `txn` batching.

## How to cross-check

Each plan doc has a task table: **kanban id · title · depends_on · acceptance
one-liner**. Run `kanban list tasks --filter '$<project-id>'` (or open the
board) and confirm: every task in the project appears here, deps match, and no
task is orphaned. Tallies: **29 tasks across 7 command plans**, plus 2 related
tasks in other projects (`plugin-arch` idempotent registration; `spatial-nav`
focus MCP server).
