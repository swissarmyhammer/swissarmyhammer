---
assignees:
- claude-code
depends_on:
- 01KS36KNHH9BPC82MFMGTY3T5J
- 01KS5F7BR6850RKT67X4CNHPAZ
position_column: todo
position_ordinal: 9c80
project: command-events
title: 'MCP notification surface: store-keyed change events for all stored things (not just entities)'
---
## What

Give the event system an MCP face so any client — the kanban webview AND AI agents — subscribes to the same change stream over MCP. The model has **four planes**, all woven together by **transaction correlation (`txn`) + provenance (`origin`)** on every notification. The schema is **generic over "stored things," not entity-specific** — there are three `TrackedStore` categories (verified) and "entity" is only one:

- **Entities** — task/tag/column/project/actor (`EntityTypeStore`). Rich **field-level** `EntityEvent`s via `broadcast::Sender<EntityEvent>` (`cache.rs:175`).
- **Views** — `ViewStore` (`views/store.rs`). Tracked + undoable, NOT an entity; no field-level diff today.
- **Perspectives** — `PerspectiveStore` (`swissarmyhammer-perspectives`). Same.
- Plus stored-but-not-tracked: **UIState** (own JSON, not undoable) and `undo_stack.yaml`.

## The four notification planes

### 1. Data changes (store-keyed) — what changed
```
notifications/store/changed {
  store, item, op: "created"|"removed"|"updated",
  changes?: [{ field, value }],   // entities carry field diffs; views/perspectives omit → reload item
  txn,                            // correlation: the transaction/undo-group this change belongs to
  origin                          // provenance: "user" | "agent:<id>" | "undo" | "redo" | "watcher"
}
```
Entity stores populate `changes` (value:null = removed); views/perspectives omit it (reload-item) until `single-changelog` unifies diff formats.

### 2. Action/command events (semantic) — what was done
```
notifications/commands/executed { id, ctx, result, txn, origin }
```
Emitted by the Command service after a successful `execute` (see the execute-verb task). Lets reactive (Obsidian-style) plugins subscribe to intent — "on task created", "on perspective switched" — not raw diffs. Shares the `txn` with the data changes the command produced, so a consumer can correlate "this action → these data changes."

### 3. Registry / lifecycle
- `notifications/commands/changed` — command registry changed (palette refresh); already in command-service.md.
- `notifications/tools/list_changed`, server-registry, plugin load/unload, board opened/closed/switched — platform/lifecycle.

### 4. Ephemeral UI state (not a stored thing, not undoable)
```
notifications/ui_state/changed { window?, key, value }   // palette/inspector/mode/focus/drag
```
Plus `notifications/store/undo_changed { can_undo, can_redo, undo_label?, redo_label? }` (stack state, from the change-propagation task).

## Correlation + provenance (cross-cutting)

- **`txn`** is the ambient transaction id from the `store` server task — the same id that groups undo entries. The Command service generates one per `execute`, propagates it via `RequestContext::extensions`; store writes stamp both the undo `group_id` AND the emitted change events with it. Consumers coalesce by `txn` → one atomic UI update per command; undo emits the inverse batch under a new `txn`.
- **`origin`** is derived from `CallerId` (already threaded) plus an undo/redo/watcher marker. Enables attribution and echo-suppression for multi-client/agent scenarios.

## Mechanism

In-process broadcast channel stays the **bus**. A **notification bridge** subscribes (entity `EntityEvent` + store `ChangeEvent` + ui_state + command-service action events), normalizes to the schemas above with `txn`/`origin`, and fans out as MCP server→client `notifications/…` over every transport (in-process for the webview/host; stdio/URL for agents). Per-client subscription registry.

Files:
- New notification-bridge module (host/plugin layer)
- `crates/swissarmyhammer-plugin/src/...` — server→client notification delivery + subscription registry; thread `txn`/`origin` through dispatch
- `crates/swissarmyhammer-tools/src/mcp/server.rs` — wire at bootstrap

## Acceptance Criteria
- [ ] One generic `store/changed { store, item, op, changes?, txn, origin }` covers entities AND views AND perspectives
- [ ] `commands/executed { id, ctx, result, txn, origin }` fires after each successful command execute
- [ ] Every notification carries `txn` and `origin`; a command's N data changes share the command's `txn`; undo emits the inverse set under a new `txn` with `origin:"undo"`
- [ ] `commands/changed`, `ui_state/changed`, `store/undo_changed` are distinct families
- [ ] In-process AND external (stdio/URL) MCP clients receive the stream
- [ ] Bridge sources from existing buses — no duplicate event production

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/mcp_notifications_e2e.rs` — subscribe in-process client; execute a multi-write command; assert (a) `commands/executed` fires, (b) all its `store/changed` events share that command's `txn`, (c) `origin:"user"`; edit a perspective → `store/changed{store:"perspective"}` no `changes`; toggle palette → `ui_state/changed`
- [ ] correlation test: `store.undo` the command; assert the inverse `store/changed` batch shares one new `txn` with `origin:"undo"`
- [ ] external CliServer client receives the same stream
- [ ] `cargo test -p swissarmyhammer-command-service --test integration mcp_notifications_e2e` passes

## Workflow
- Use `/tdd` — write the correlation test (one command → many `store/changed` sharing a `txn` + a `commands/executed`) first; it pins the model.

Prerequisite for change-propagation and frontend-migration. Depends on the `store` server (txn id) and plugin-platform transport. Coordinates with `single-changelog`.