---
assignees:
- claude-code
depends_on:
- 01KTCAEXMCAWWTE7FBGP7BE86Z
position_column: todo
position_ordinal: '8880'
project: card-comments
title: Normalize comment-log field writes in UpdateEntityField (UI field-set path)
---
## What
Make the UI field-set path work for comments. The UI never dispatches comment ops — editors are pure UI; `Field` commits the whole field value through the generic `entity.update_field` command → `UpdateEntityField` op (`crates/swissarmyhammer-kanban/src/entity/update_field.rs:78`). So when the comment editor (separate UI task) commits a new `comments` array, `UpdateEntityField` must normalize it server-side. This keeps all comment/actor logic in the **kanban layer** — the generic `swissarmyhammer-entity` crate stays unaware of comments/actors.

CONFIRMED design hook: `UpdateEntityField::execute` already branches on field type — `FieldType::Computed` routes through a `DeriveHandler` (update_field.rs:104-139), else direct read-set-write (update_field.rs:142-152). Add a new branch for `FieldType::CommentLog` BEFORE the normal path.

Add `crates/swissarmyhammer-kanban/src/comment/normalize.rs` (or a fn in the comment module) `pub(crate) async fn normalize_comment_log(ctx, old: &Value, incoming: &Value) -> Result<Value>` and call it from the new `UpdateEntityField` branch. Algorithm — diff incoming array against the existing stored array, keyed on member `id` ("new member = has no `id`"):
- **New members** (incoming member with no `id`, or an `id` not present in old): assign a fresh ulid id + now() ISO 8601 timestamp; resolve the author via `resolve_comment_author(ctx, member.actor.as_deref())` (reuse the task-3 helper: explicit actor validated, else OS-user fallback). Build via `build_comment_member`.
- **Existing members** (incoming `id` matches an old member): preserve the old `actor` and `timestamp` (immutable); take only the new `text` (text-only edit, per design).
- **Deleted members** (old `id` absent from incoming): dropped — deletion is allowed.
- Result is the normalized array, sorted by timestamp ascending; the branch writes it via `ectx.write(&entity)` and returns the entity JSON (same shape as the other branches).

Reuse `build_comment_member` and `resolve_comment_author` from the comment module — do NOT duplicate member/author logic. The `AddComment` op and this branch must produce identical member shapes.

Files:
1. `crates/swissarmyhammer-kanban/src/entity/update_field.rs` — add the `FieldType::CommentLog` branch (mirror the Computed branch structure: read field_def, read entity, normalize, write, return).
2. `crates/swissarmyhammer-kanban/src/comment/normalize.rs` (+ `mod.rs` re-export) — the `normalize_comment_log` diff function.

## Acceptance Criteria
- [ ] `UpdateEntityField` on a `comments` field routes through comment-log normalization, not the plain set path.
- [ ] Committing an array with a member lacking `id` assigns id+timestamp+author (author = explicit if given and valid, else OS-user fallback).
- [ ] Committing an array where an existing member's `text` changed keeps its original `actor` and `timestamp` and updates only `text`.
- [ ] Omitting a previously-stored member from the committed array deletes it.
- [ ] Normalized members are byte-identical in shape to those produced by the `AddComment` op (shared helpers).
- [ ] Author/actor/OS-user logic appears only in the kanban crate; `swissarmyhammer-entity` is untouched.
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] `normalize.rs` unit tests on the pure diff: new-member (no id) → gets id/timestamp/author; existing-member text change → who/when preserved, text updated; removed-member → dropped; ordering by timestamp.
- [ ] Integration test through `UpdateEntityField::execute` against a temp board: init board + task, commit `comments=[{text:"hi"}]` → re-read task, assert one member with id/actor(=OS-user)/timestamp; commit again with that member's text edited + a second new member → assert first member's actor/timestamp unchanged and second member added; commit with a member removed → assert it is gone.
- [ ] A test that an explicit unknown actor on a new member errors (reuses resolve_comment_author validation).
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the diff unit tests + the UpdateEntityField integration test first, then implement the branch.