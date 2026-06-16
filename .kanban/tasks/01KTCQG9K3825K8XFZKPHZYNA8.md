---
assignees:
- claude-code
depends_on:
- 01KTCAEXMCAWWTE7FBGP7BE86Z
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa480
project: card-comments
title: Normalize comment-log field writes in UpdateEntityField (UI field-set path)
---
## What
Make the UI field-set path work for comments. The UI never dispatches comment ops — editors are pure UI; `Field` commits the whole field value through the generic `entity.update_field` command → `UpdateEntityField` op (`crates/swissarmyhammer-kanban/src/entity/update_field.rs:78`). So when the comment editor (separate UI task) commits a new `comments` array, `UpdateEntityField` must normalize it server-side. This keeps all comment/actor logic in the **kanban layer** — the generic `swissarmyhammer-entity` crate stays unaware of comments/actors.

CONFIRMED design hook: `UpdateEntityField::execute` already branches on field type — `FieldType::Computed` routes through a `DeriveHandler` (update_field.rs:104-139), else direct read-set-write (update_field.rs:142-152). Add a new branch for `FieldType::CommentLog` BEFORE the normal path.

Add `crates/swissarmyhammer-kanban/src/comment/normalize.rs` (or a fn in the comment module) `pub(crate) async fn normalize_comment_log(ctx, old: &Value, incoming: &Value) -> Result<Value>` and call it from the new `UpdateEntityField` branch.

### Merge semantics — NOT diff-as-delete (protects concurrent agent appends)
The conversation log has a concurrent writer by design: the agent appends progress comments while a user may have the same task's inspector open. The UI commits a whole array built from a possibly-stale snapshot, so **an old member being absent from the incoming array must NOT mean deletion** — otherwise a comment the agent appended between the user's last render and their commit gets silently dropped. Deletion is only ever EXPLICIT, via a tombstone member `{ "id": <id>, "deleted": true }` emitted by the editor (separate UI task). Tombstones are a wire-only convention between the editor and this normalize step — they are never stored.

Algorithm — merge incoming against the existing stored array, keyed on member `id`:
- **New members** (incoming member with no `id`): assign a fresh ulid id + now() UTC RFC3339 timestamp; resolve the author via `resolve_comment_author(ctx, member.actor.as_deref())` (reuse the task-3 helper: explicit actor validated, else OS-user fallback). Build via `build_comment_member`. An incoming non-tombstone member whose `id` does not exist in old is ALSO treated as new (its supplied id is discarded; fresh id assigned).
- **Existing members** (incoming `id` matches an old member): preserve the old `actor` and `timestamp` (immutable); take only the new `text` (text-only edit, per design).
- **Tombstones** (incoming `{id, deleted: true}`): remove that member from the result. A tombstone for an unknown id is a no-op.
- **Old members absent from incoming: PRESERVED** (the concurrent-append protection).
- Result is the normalized array sorted by member `id` ascending (ULIDs are time-ordered, so id order == creation order, and ids are unique — no same-millisecond timestamp ties); the branch writes it via `ectx.write(&entity)` and returns the entity JSON (same shape as the other branches).

Reuse `build_comment_member` and `resolve_comment_author` from the comment module — do NOT duplicate member/author logic. The `AddComment` op and this branch must produce identical member shapes.

Files:
1. `crates/swissarmyhammer-kanban/src/entity/update_field.rs` — add the `FieldType::CommentLog` branch (mirror the Computed branch structure: read field_def, read entity, normalize, write, return).
2. `crates/swissarmyhammer-kanban/src/comment/normalize.rs` (+ `mod.rs` re-export) — the `normalize_comment_log` merge function.

## Acceptance Criteria
- [ ] `UpdateEntityField` on a `comments` field routes through comment-log normalization, not the plain set path.
- [ ] Committing an array with a member lacking `id` assigns id+timestamp+author (author = explicit if given and valid, else OS-user fallback); timestamp is UTC RFC3339.
- [ ] Committing an array where an existing member's `text` changed keeps its original `actor` and `timestamp` and updates only `text`.
- [ ] A tombstone `{id, deleted: true}` deletes that member; tombstones are never stored.
- [ ] An old member ABSENT from the committed array is PRESERVED (concurrent-append protection) — absence is not deletion.
- [ ] Normalized members are byte-identical in shape to those produced by the `AddComment` op (shared helpers).
- [ ] Author/actor/OS-user logic appears only in the kanban crate; `swissarmyhammer-entity` is untouched.
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] `normalize.rs` unit tests on the pure merge: new-member (no id) → gets id/timestamp/author; existing-member text change → who/when preserved, text updated; tombstone → removed; tombstone for unknown id → no-op; **absent old member → preserved** (simulate the race: old contains an agent comment the incoming snapshot lacks; assert it survives); incoming unknown non-tombstone id → treated as new with fresh id; result ordered by id ascending.
- [ ] Integration test through `UpdateEntityField::execute` against a temp board: init board + task, commit `comments=[{text:"hi"}]` → re-read task, assert one member with id/actor(=OS-user)/timestamp; then `AddComment` an agent comment AND commit a stale UI array (text edit of the first member, agent comment absent) → assert the agent comment survives and the edit applied; commit with a tombstone for the first member → assert it is gone, agent comment remains.
- [ ] A test that an explicit unknown actor on a new member errors (reuses resolve_comment_author validation).
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the merge unit tests + the UpdateEntityField integration test first, then implement the branch.