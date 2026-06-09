---
assignees:
- claude-code
depends_on:
- 01KTCAFH74MPPZ9282P699QBW0
- 01KTCQG9K3825K8XFZKPHZYNA8
position_column: todo
position_ordinal: '8580'
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
- Add integration tests where existing EntityChanged-broadcast tests live (find the test near `context.rs` ~line 3927 "Exactly one EntityChanged event"). Subscribe to the entity cache broadcast channel, run the comment mutation through the real path, assert exactly one `EntityChanged` whose `changes` contains a `FieldChange` for `field == "comments"` with the expected member present in `value`.

## Acceptance Criteria
- [ ] Agent path: `AddComment` produces exactly one `EntityChanged` with a `comments` `FieldChange`; edit and delete each produce one too.
- [ ] UI path: `UpdateEntityField` committing a new `comments` array produces a `comments` `FieldChange` carrying the normalized array.
- [ ] The event is thin: `{field, value}` only — no enrichment round-trip / extra reads introduced.
- [ ] Any production change required to make the event fire is minimal and documented in the test comments.

## Tests
- [ ] `comment_add_emits_field_change_event` (agent op path).
- [ ] `comment_edit_and_delete_emit_field_change_events` (agent op path).
- [ ] `comment_field_set_emits_field_change_event` (UI `UpdateEntityField` path).
- [ ] `cargo nextest run -p swissarmyhammer-kanban` (and `-p swissarmyhammer-entity` if a test lives there) — green.

## Workflow
- Use `/tdd` — write the subscribe-and-assert tests first; they should pass once the comment paths exist (dependencies), confirming the loop. If one fails, fix the production path minimally.