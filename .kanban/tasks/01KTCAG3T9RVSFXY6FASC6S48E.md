---
assignees:
- claude-code
depends_on:
- 01KTCAFH74MPPZ9282P699QBW0
- 01KTCQG9K3825K8XFZKPHZYNA8
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa780
project: card-comments
title: Verify comment-add emits a field-level event and lands in the entity store (production-path test)
---
## What
Comments mutate the task's `comments` field via `ectx.write(&task)`, so the entity cache already emits `EntityEvent::EntityChanged` carrying a `FieldChange { field: "comments", value: <new array> }` on its broadcast channel. CONFIRMED shape: `crates/swissarmyhammer-entity/src/events.rs` defines `EntityEvent::EntityChanged { changes: Vec<FieldChange> }` (events.rs:38-48), FieldChange is the thin field-level shape (events.rs:21), removals encoded as `FieldChange { value: Null }`. No NEW event type is needed — this task PROVES the thin field-level event fires for comment changes via BOTH write paths and reaches a subscriber, closing the command → event → store loop the project requires for every new store-affecting feature.

There are two write paths and the test must cover both:
- **Agent op path**: `AddComment` / `UpdateComment` / `DeleteComment` (depends on the agent-dispatch task).
- **UI field-set path**: `UpdateEntityField` on the `comments` field (depends on the normalization task) — this is the path the React editor actually exercises.

This task is a real-path integration test, not new production code (add production code only if the test reveals the event does NOT fire — e.g. a no-op-write suppression that swallows the comments change).

Files:
- Added `crates/swissarmyhammer-kanban/tests/comment_event_broadcast.rs` — integration tests subscribing to the real `EntityCache` broadcast channel wired by `KanbanContext::entity_context()`, running comment mutations through the real command paths, asserting exactly one `EntityChanged` whose `changes` contains the `comments` `FieldChange` with the expected member in `value`. (Tests live in the kanban crate rather than next to the entity-crate broadcast tests because the real path requires `KanbanContext` + the comment ops, which the entity crate cannot depend on.)

## Acceptance Criteria
- [x] Agent path: `AddComment` produces exactly one `EntityChanged` with a `comments` `FieldChange`; edit and delete each produce one too.
- [x] UI path: `UpdateEntityField` committing a new `comments` array produces a `comments` `FieldChange` carrying the normalized array (ULID id, resolved author, UTC RFC3339 timestamp).
- [x] The event is thin: `{field, value}` only — no enrichment round-trip / extra reads introduced.
- [x] Any production change required to make the event fire is minimal and documented in the test comments — NONE was needed; the existing cache diff already emits the `comments` change for every path (documented in the test file header).

## Tests
- [x] `comment_add_emits_field_change_event` (agent op path).
- [x] `comment_edit_and_delete_emit_field_change_events` (agent op path).
- [x] `comment_field_set_emits_field_change_event` (UI `UpdateEntityField` path).
- [x] `cargo nextest run -p swissarmyhammer-kanban` — 1495/1495 green (no test added to `-p swissarmyhammer-entity`; the real path lives in the kanban crate).

## Workflow
- Use `/tdd` — write the subscribe-and-assert tests first; they should pass once the comment paths exist (dependencies), confirming the loop. If one fails, fix the production path minimally. (Done: tests written first, passed on first run as the card anticipated; assertion liveness verified with a deliberate red run — payload assertion failed with the real event value `"first comment"` — then reverted.)