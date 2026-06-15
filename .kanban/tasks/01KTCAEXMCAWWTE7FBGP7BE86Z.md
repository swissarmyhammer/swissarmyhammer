---
assignees:
- claude-code
depends_on:
- 01KTCAE5WVKRHTYNJYZT7F2M9K
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa380
project: card-comments
title: Implement comment commands (add/list/get/edit/delete) operating on the inline comments field
---
## What
Create a `comment/` module in `crates/swissarmyhammer-kanban/src/` providing (a) the agent-facing operation structs that read and mutate the task's inline `comments` field, and (b) reusable helpers that the UI field-set path (separate task) also uses. The value is stored inline on the task `comments` field as a JSON array of `{id, actor, text, timestamp}` member objects — NOT as a separate entity (contrast with attachment, which creates its own `attachment` Entity). This honors the "comments are dependent members, not their own entity kind" requirement.

CONFIRMED (research): `write_internal` in `swissarmyhammer-entity/src/context.rs:967` only special-processes `FieldType::Attachment`; a `comment-log` field stores its raw JSON array faithfully and reads back unchanged (no enrichment). So append-and-write works directly on the field value.

Each comment member is a JSON object: `{ "id": <ulid lowercased>, "actor": <actor-id>, "text": <free text>, "timestamp": <UTC RFC3339 string> }`. Generate the id with `ulid::Ulid::new().to_string().to_lowercase()`. The `timestamp` is set at add time as **UTC RFC3339** via the crate's existing chrono usage (`chrono::Utc::now().to_rfc3339()` — do not hand-roll formatting, and do NOT use local-offset timestamps: boards sync across machines via git and mixed-offset ISO strings don't sort lexically).

ORDERING: the canonical order of a comment log is member `id` ascending — ULIDs are time-ordered, so id order == creation order, and ids are unique (no same-millisecond timestamp ties). Edits preserve `id`, so order is stable under edit.

### Response shapes — follow the op-token-diet convention (mutations acknowledge, they don't echo)
The landed op-token-diet project (`task_mutation_ack` in `crates/swissarmyhammer-kanban/src/task_helpers.rs`, returning `{ok: true, id: <task ulid>, short_id}`) is the house envelope for task mutations. Comment ops mutate a task, so they reuse it — top-level `id` is always the TASK id (this is what the MCP wrapper's `_plan._meta.affected_task_id` extraction reads):
- `add comment` → `task_mutation_ack(&entity)` PLUS the new member under a `comment` key: `{ok, id, short_id, comment: {id, actor, text, timestamp}}`. The member is genuinely new information (server-assigned id/timestamp/resolved author) — the `add task`-returns-slim analogue.
- `update comment` → exactly `task_mutation_ack(&entity)` = `{ok, id, short_id}`. NO member echo — echoing the updated text re-bills the tokens the agent just sent.
- `delete comment` → exactly `task_mutation_ack(&entity)` = `{ok, id, short_id}`.
- `list comments` / `get comment` are READS — they return member data (the array / one member), not acks.

### Reusable helpers (used by BOTH the agent ops here AND the UI field-set normalization task)
Expose `pub(crate)` helpers from the comment module so the `UpdateEntityField` comment-log branch (separate task) can reuse them — single source of truth for member shape + author rules:
- `build_comment_member(text, author_id) -> Value` — constructs a new member with fresh ulid id + now() UTC RFC3339 timestamp.
- `resolve_comment_author(ctx, explicit: Option<&str>) -> Result<ActorId>` — `Some` → validate the actor entity exists (clear error if missing); `None` → resolve OS-level user identity and ensure that actor exists, never erroring.

Add the OS-user resolver in the **actor domain** (not the comment module): `crates/swissarmyhammer-kanban/src/actor/` — `pub(crate) async fn ensure_os_user_actor(ctx) -> Result<ActorId>`:
- Resolve OS identity with the `whoami` crate: `whoami::username()` for the id basis (slugify/lowercase to a valid actor id) and `whoami::realname()` for the display name (fall back to username if empty/unknown).
- Idempotently ensure the actor via `AddActor::new(id, name).with_ensure()` (CONFIRMED: `actor/add.rs` ensure=true creates-or-returns). Return the actor id.
- Add `whoami = "1"` to `crates/swissarmyhammer-kanban/Cargo.toml` (already a workspace dep in `apps/kanban-app/Cargo.toml`).

### Agent-facing ops (under `crates/swissarmyhammer-kanban/src/comment/`)
Model module layout on `src/attachment/` (mod.rs + add.rs + get.rs + list.rs + update.rs + delete.rs) and the `#[operation(verb, noun)]` macro pattern.
1. `mod.rs` — declare submodules; re-export the five command structs, `build_comment_member`, `resolve_comment_author`, and a `comment_member_to_json` helper.
2. `add.rs` — `AddComment { task_id, actor: Option<String>, text }`, `#[operation(verb="add", noun="comment")]`. Reads the task, resolves author via `resolve_comment_author`, appends `build_comment_member(...)`, writes the task back with `task.set("comments", ...)`. Returns `{ok, id, short_id, comment: <new member>}` per the response-shapes section. MUST preserve existing members.
3. `list.rs` — `ListComments { task_id }`, `#[operation(verb="list", noun="comments")]`. Returns the comments array sorted by member `id` ascending (creation order — see ORDERING above).
4. `get.rs` — `GetComment { task_id, id }` → one member or `KanbanError::CommentNotFound { id }` (variant ALREADY EXISTS in error.rs).
5. `update.rs` — `UpdateComment { task_id, id, text }` edits only `text` (who/when immutable). Returns the pure ack `{ok, id, short_id}`. CommentNotFound if absent.
6. `delete.rs` — `DeleteComment { task_id, id }` removes by id. Returns the pure ack `{ok, id, short_id}`. CommentNotFound if absent. (Tombstones are NOT part of this surface — they are a wire-only convention of the UI field-set path; the agent deletes explicitly via this op.)
7. Wire `pub mod comment;` into `src/lib.rs` (next to `attachment`).

Note: the MCP agent always dispatches with an explicit actor (`claude-code`), so the OS-user fallback only triggers for actor-less callers — it never mis-attributes agent comments.

## Acceptance Criteria
- [x] Five ops exist with correct `#[operation]` verb/noun annotations; `AddComment.actor` is `Option<String>`.
- [x] Response shapes follow the op-token-diet convention via `task_mutation_ack`: `add comment` = ack + `comment` member; `update comment`/`delete comment` = exactly `{ok, id, short_id}` (task identity, no member echo); `list`/`get` return member data.
- [x] `build_comment_member` and `resolve_comment_author` are `pub(crate)` and reusable; `ensure_os_user_actor` lives in the actor module.
- [x] A member stores who/what/when + a stable ulid id; `timestamp` is UTC RFC3339; adding preserves existing members.
- [x] `list comments` returns members in id order (creation order).
- [x] explicit existing actor attributed; explicit non-existent actor errors; `actor: None` resolves+ensures the OS-user actor and succeeds.
- [x] get/update/delete on a missing id return `KanbanError::CommentNotFound`; update changes text only.
- [x] No `comment` Entity is ever created (assert no comment file under the tasks dir).
- [x] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [x] `add.rs` tests (modeled on `attachment/add.rs`): add two comments (explicit actor), re-read via `list comments`, assert 2 members w/ actor/text/timestamp/id, that the timestamp parses as RFC3339 UTC, and that the second add preserved the first. Assert the add RESPONSE shape with `assert_task_mutation_ack_with(result, task_id, &["comment"])` (the shared `#[cfg(test)]` helper in `task_helpers.rs` from op-token-diet).
- [x] `update.rs`/`delete.rs` tests assert the pure ack via `assert_task_mutation_ack` and verify the effect (text changed / member gone) via `list comments`/`get comment` — stored state, not response echo.
- [x] Explicit unknown actor errors; `actor: None` path writes a member whose `actor` is the ensured OS-user id and that actor entity now exists (idempotent on repeat).
- [x] `list comments` returns members sorted by id ascending.
- [x] get/update/delete happy paths + CommentNotFound for a bogus id; update leaves actor/timestamp unchanged.
- [x] Assert no standalone comment entity file is written.
- [x] `cargo nextest run -p swissarmyhammer-kanban comment` — green.

## Workflow
- Use `/tdd` — write add+preserve, author-resolution paths, response-shape, and CommentNotFound tests first, then implement.

## Implementation note (done)
Implemented via TDD (RED: 19 tests failing on `todo!()` skeletons → GREEN: all pass). One deviation from the card's ORDERING note: `ulid::Ulid::new()` is random in its low bits within the same millisecond, so same-ms ids are unique but not strictly ordered; the id-ascending guarantee (and its test) holds across millisecond boundaries. Full crate suite: 1474/1474 green; clippy `-D warnings` clean.