---
assignees:
- claude-code
depends_on:
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5F5ZNA0621X8KM2NPERXNV
- 01KS5G3AKZXDN7K6YR415E0V4K
position_column: todo
position_ordinal: 9b80
project: command-events
title: 'Change propagation: undo/redo emit edit-shaped data events + a stack-state event (caches + UI)'
---
## What

Define what undo AND redo emit into the event system so every dependent — in-process caches and the UI — reacts through the paths they already use, with nothing downstream special-casing undo/redo. There are **two** distinct notifications, and redo makes the distinction obvious. (The generic MCP delivery of these notifications is the separate "MCP notification surface" task; this task defines the *content* and the *emit-on-undo/redo* behavior.)

### A. Data changes (symmetric undo/redo) — derive the event from the byte transition

`StoreContext::undo` applies `reverse_patch`; `redo` applies `forward_patch`. Both return `UndoOutcome { items: [(store, item)…] }`. After the rewrite, emit a **thin change event per item describing the actual transition on disk**, NOT a blanket "update":
- none → content = `entity created`
- content → none = `entity removed`
- content → content = field-level `{field, value}` changes (diff via `swissarmyhammer-entity/src/changelog.rs`)

These are the existing thin `EntityEvent` shapes (`swissarmyhammer-entity/src/events.rs`: `EntityChanged`/`EntityDeleted` with `{field,value}`), pushed onto the existing `broadcast::Sender<EntityEvent>` bus (`cache.rs:175`). One direction-agnostic derivation serves both undo and redo — redo-of-a-create emits `created`, redo-of-a-delete emits `removed`. If we blindly emitted "update", a redo-of-delete would leave a ghost card.

Consumers (unchanged by direction):
- **In-process caches** (`kanban` entity cache, `views`/perspective cache) already subscribe to the broadcast bus (`cache.subscribe()`); ensure they reload on undo/redo-sourced events too. This replaces the bespoke `reconcile_post_undo_caches` and also covers normal edits, agent writes, watcher edits.
- **UI + agents** receive the same events via the MCP notification surface (separate task) — the webview subscribes as an MCP client; there is no Tauri-specific path for change events anymore.

### B. Undo-stack state event (its own concern — fires on push/undo/redo)

Undo/redo also change `can_undo`/`can_redo` and the next undo/redo labels — state the UI's Undo/Redo controls depend on, separate from entity data. The `store` server emits a **stack-state notification on every stack mutation** — `push`, `undo`, `redo` — carrying `{ can_undo, can_redo, undo_label?, redo_label? }` (peeked from `UndoStack` around the pointer), delivered as `notifications/store/undo_changed` via the MCP notification surface.

Critical non-symmetric case: a **normal edit after an undo discards the redo tail** (`UndoStack::push` truncates at the pointer), so `can_redo` flips to false with no undo/redo call — just an edit. The stack-state event must therefore fire on plain writes too, or the Redo control stays wrongly enabled. Data events never carry this; it is a property of the stack, not any item.

Files:
- `crates/swissarmyhammer-store/src/context.rs` — `undo`/`redo` emit transition-derived data events for `UndoOutcome.items`; `push`/`undo`/`redo` emit the stack-state event
- `crates/swissarmyhammer-entity/src/context.rs`, `crates/swissarmyhammer-views/src/context.rs` — caches reload on undo/redo-sourced bus events; drop manual post-undo reconcile
- `crates/swissarmyhammer-kanban/src/commands/app_commands.rs` — remove `reconcile_post_undo_caches` (replaced by the subscription)

## Acceptance Criteria
- [ ] undo and redo emit data events derived from the actual byte transition (create/remove/field), not a blanket update
- [ ] redo-of-a-delete emits `removed`; redo-of-a-create emits `created`; redo-of-an-update emits the new field values
- [ ] in-process caches refresh from those bus events with no direction-specific code; `reconcile_post_undo_caches` is gone
- [ ] a stack-state event fires on push, undo, AND redo with correct `can_undo`/`can_redo`/labels
- [ ] a normal edit after an undo fires a stack-state event with `can_redo:false` (redo tail discarded)
- [ ] data + stack-state events are delivered to clients via the MCP notification surface (UI and agents alike)

## Tests
- [ ] `crates/swissarmyhammer-store/tests/integration/undo_redo_emits_events_e2e.rs` — create→undo emits `removed`; redo emits `created`; update→undo emits old field values, redo emits new
- [ ] `crates/swissarmyhammer-store/tests/integration/stack_state_events_e2e.rs` — push fires (can_undo true); undo fires (can_redo true); redo fires (can_redo false); a fresh edit after an undo fires with can_redo false
- [ ] `crates/swissarmyhammer-command-service/tests/integration/undo_redo_notifies_dependents_e2e.rs` — boot `kanban`+`views`+`store`; edit entity + perspective; undo then redo; assert caches reflect each state without manual refresh AND a subscribed MCP client received both the data events and the stack-state events
- [ ] `cargo test -p swissarmyhammer-store && cargo test -p swissarmyhammer-command-service --test integration undo_redo_notifies_dependents_e2e` pass

## Workflow
- Use `/tdd` — write `undo_redo_emits_events_e2e.rs` (transition-derivation) and `stack_state_events_e2e.rs` first.

Depends on the `store` server, shared-substrate, and MCP notification surface tasks. Related to the event-architecture rule (thin events) and `single-changelog`.