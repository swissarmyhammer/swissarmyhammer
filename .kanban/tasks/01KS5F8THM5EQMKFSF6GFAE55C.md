---
assignees:
- claude-code
depends_on:
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5F5ZNA0621X8KM2NPERXNV
- 01KS5G3AKZXDN7K6YR415E0V4K
position_column: review
position_ordinal: '80'
project: command-events
title: 'Change propagation: undo/redo emit edit-shaped data events + a stack-state event (caches + UI)'
---
## What

Make undo/redo propagate to in-process caches **and** the UI through the *same* paths a normal edit uses, with nothing downstream special-casing undo/redo, and add the one event that genuinely doesn't exist yet (undo-stack state). (Generic MCP delivery of these notifications is the separate "MCP notification surface" task; this task is about *content* and *emit-on-undo/redo* behavior.)

### Current state — READ THIS FIRST (the convergence already exists)

"Undo == edit downstream" is **already true today**, do not rebuild it:
- `StoreContext::undo`/`redo` (`crates/swissarmyhammer-store/src/context.rs`) only rewrite disk via `reverse_patch`/`forward_patch` and return `UndoOutcome { items: [(store, item)…] }`. They do **NOT** emit entity/view/perspective events and **cannot**: `swissarmyhammer-store` has no dependency on `-entity`/`-views`/`-kanban` and no broadcast sender (Cargo.toml = diffy/serde/tokio only). Emitting entity field-diffs from store would be a layering inversion — do not do it.
- One layer up, `crates/swissarmyhammer-kanban/src/commands/app_commands.rs::reconcile_post_undo_caches` (called after every `undo`/`redo`) already reconciles **all three** store categories, and each branch derives the transition and emits on the same broadcast bus a normal edit / watcher edit uses:
  - **Entities**: iterates `outcome.items` → `EntityContext::sync_entity_cache_from_disk` (`context.rs:658`) → `cache.refresh_from_disk` (exists → field diff via `cache.rs::diff` → `EntityChanged`) or `cache.evict` (gone → `EntityDeleted`). Same two calls the file watcher uses (`watcher.rs:210`).
  - **Perspectives**: `reconcile_perspective_cache` → `pctx.reload_from_disk(item)` → `PerspectiveChanged`/`PerspectiveDeleted`.
  - **Views**: `reconcile_view_cache` → `views.reload_from_disk(item)` → `ViewEvent`.
- redo-of-a-delete already surfaces as `removed` and redo-of-a-create as `created`, because the event is derived from the post-rewrite byte state, not from the direction. Validated by `crates/swissarmyhammer-entity/src/context.rs:4078` (delete→undo emits `EntityChanged`, redo emits `EntityDeleted`) and `:4115` (archive round-trip). Idempotency: the watcher also sees undo's disk writes, but `refresh_from_disk`'s hash gate makes that a silent no-op — no double-fire.

So the data-event path is correct and converged. The real work is (A) **unify** the reconcile, (B) add `txn`/`origin`, (C) add the **stack-state** event.

### A. Data changes — keep the reconcile, unify it, don't delete it

`reconcile_post_undo_caches` is the convergence point. Removing it would leave nobody to emit (store can't). The work is to collapse its three hand-written, store-name-keyed branches into one uniform reconcile keyed on `outcome.items` + the store's category, so a new store category doesn't require a new bespoke branch. The per-item derivation (none→content = created, content→none = removed, content→content = field `{field,value}`) already lives in `refresh_from_disk`/`reload_from_disk`; reuse it.

Consumers (unchanged by direction): in-process caches re-read on the reconcile-emitted bus events (also covers normal edits, agent writes, watcher edits); UI + agents receive the same events via the MCP notification surface (separate task) — the webview subscribes as an MCP client, no Tauri-specific change path remains.

### B. `txn` + `origin` on data events (net-new field work)

None of `EntityEvent` (`crates/swissarmyhammer-entity/src/events.rs:31`), `PerspectiveChanged`, or `ViewEvent` carry correlation/provenance today. Add `txn` (ambient transaction id opened by the Command engine's `execute`, propagated via `RequestContext::extensions`) and `origin` (`user|agent:id|undo|redo|watcher`) so the UI batches a command's N changes into one atomic re-render and can attribute/echo-suppress. The reconcile stamps `origin: undo|redo`; the watcher stamps `origin: watcher`.

### C. Undo-stack state event (its own concern — the only truly new event)

Undo/redo/push also change `can_undo`/`can_redo` and the next undo/redo labels — UI control state, separate from item data and not carried by any data event. The `store` server emits a stack-state notification on **every** stack mutation — `push`, `undo`, `redo` — carrying `{ can_undo, can_redo, undo_label?, redo_label? }` (peeked from `UndoStack` around the pointer), delivered as `notifications/store/undo_changed`. This event CAN originate in `swissarmyhammer-store` (it owns `UndoStack` and the event carries no foreign types) — adding a stack-state sender to the store is in-layer, unlike data events.

Critical non-symmetric case: a **normal edit after an undo discards the redo tail** (`UndoStack::push` truncates at the pointer), so `can_redo` flips to false with no undo/redo call — just an edit. The stack-state event must therefore fire on plain `push` too, or the Redo control stays wrongly enabled.

Files:
- `crates/swissarmyhammer-kanban/src/commands/app_commands.rs` — unify `reconcile_post_undo_caches`'s three branches into one category-keyed reconcile; stamp `origin: undo|redo`. (Do NOT delete the reconcile; it is the emit trigger. If/when it relocates into the store-service/entity-service wiring, the mechanism must survive intact.)
- `crates/swissarmyhammer-entity/src/events.rs`, perspective/view event types — add `txn`/`origin` fields.
- `crates/swissarmyhammer-entity/src/cache.rs`, `crates/swissarmyhammer-views/src/context.rs` — thread `txn`/`origin` through `refresh_from_disk`/`reload_from_disk` emission; stamp `origin: watcher` on watcher-sourced refresh.
- `crates/swissarmyhammer-store/src/context.rs` — `push`/`undo`/`redo` emit the stack-state event (new in-layer stack-state sender); undo/redo continue to return `UndoOutcome` for the reconcile (no entity emission added here).

## Acceptance Criteria
- [ ] undo and redo data events are derived from the actual byte transition (create/remove/field) — confirmed already true and covered by a regression test, not newly invented in `swissarmyhammer-store`
- [ ] redo-of-a-delete emits `removed`; redo-of-a-create emits `created`; redo-of-an-update emits the new field values
- [ ] `reconcile_post_undo_caches`'s three branches are unified into one category-keyed reconcile; the reconcile is retained as the emit trigger (NOT removed); a new store category needs no new bespoke branch
- [ ] `EntityEvent`/perspective/view events carry `txn` + `origin`; a single command's N changes share one `txn`; undo/redo-sourced events carry `origin: undo`/`redo`, watcher-sourced carry `origin: watcher`
- [ ] no entity/view/perspective event emission is added inside `swissarmyhammer-store` (layering preserved); store emits only the stack-state event
- [ ] a stack-state event fires on push, undo, AND redo with correct `can_undo`/`can_redo`/labels
- [ ] a normal edit after an undo fires a stack-state event with `can_redo:false` (redo tail discarded)
- [ ] data + stack-state events are delivered to clients via the MCP notification surface (UI and agents alike)

## Tests
- [ ] `crates/swissarmyhammer-store/tests/integration/stack_state_events_e2e.rs` — push fires (can_undo true); undo fires (can_redo true); redo fires (can_redo false); a fresh edit after an undo fires with can_redo false
- [ ] `crates/swissarmyhammer-entity/tests/integration/undo_redo_emits_transition_events_e2e.rs` — create→undo emits `removed`; redo emits `created`; update→undo emits old field values, redo emits new; each carries `origin: undo|redo` and a shared `txn` per command (exercises the reconcile path, NOT a store-level emitter)
- [ ] `crates/swissarmyhammer-command-service/tests/integration/undo_redo_notifies_dependents_e2e.rs` — boot `kanban`+`views`+`store`; edit entity + perspective + view; undo then redo; assert caches reflect each state via the unified reconcile (no per-category bespoke code) AND a subscribed MCP client received both the data events (with txn/origin) and the stack-state events
- [ ] `cargo test -p swissarmyhammer-store && cargo test -p swissarmyhammer-entity && cargo test -p swissarmyhammer-command-service --test integration undo_redo_notifies_dependents_e2e` pass

## Workflow
- Use `/tdd` — write `stack_state_events_e2e.rs` and `undo_redo_emits_transition_events_e2e.rs` first.

Depends on the `store` server, shared-substrate, and MCP notification surface tasks. Related to the event-architecture rule (thin events) and `single-changelog`.