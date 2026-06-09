---
assignees:
- claude-code
depends_on:
- 01KTCAE5WVKRHTYNJYZT7F2M9K
position_column: todo
position_ordinal: '8380'
project: card-comments
title: Implement comment commands (add/list/get/edit/delete) operating on the inline comments field
---
## What
Create a `comment/` module in `crates/swissarmyhammer-kanban/src/` providing (a) the agent-facing operation structs that read and mutate the task's inline `comments` field, and (b) reusable helpers that the UI field-set path (separate task) also uses. The value is stored inline on the task `comments` field as a JSON array of `{id, actor, text, timestamp}` member objects — NOT as a separate entity (contrast with attachment, which creates its own `attachment` Entity). This honors the "comments are dependent members, not their own entity kind" requirement.

CONFIRMED (research): `write_internal` in `swissarmyhammer-entity/src/context.rs:967` only special-processes `FieldType::Attachment`; a `comment-log` field stores its raw JSON array faithfully and reads back unchanged (no enrichment). So append-and-write works directly on the field value.

Each comment member is a JSON object: `{ "id": <ulid lowercased>, "actor": <actor-id>, "text": <free text>, "timestamp": <ISO 8601 string> }`. Generate the id with `ulid::Ulid::new().to_string().to_lowercase()`. The `timestamp` is an ISO 8601 string set at add time via the crate's existing chrono usage (do not hand-roll formatting).

### Reusable helpers (used by BOTH the agent ops here AND the UI field-set normalization task)
Expose `pub(crate)` helpers from the comment module so the `UpdateEntityField` comment-log branch (separate task) can reuse them — single source of truth for member shape + author rules:
- `build_comment_member(text, author_id) -> Value` — constructs a new member with fresh ulid id + now() ISO 8601 timestamp.
- `resolve_comment_author(ctx, explicit: Option<&str>) -> Result<ActorId>` — `Some` → validate the actor entity exists (clear error if missing); `None` → resolve OS-level user identity and ensure that actor exists, never erroring.

Add the OS-user resolver in the **actor domain** (not the comment module): `crates/swissarmyhammer-kanban/src/actor/` — `pub(crate) async fn ensure_os_user_actor(ctx) -> Result<ActorId>`:
- Resolve OS identity with the `whoami` crate: `whoami::username()` for the id basis (slugify/lowercase to a valid actor id) and `whoami::realname()` for the display name (fall back to username if empty/unknown).
- Idempotently ensure the actor via `AddActor::new(id, name).with_ensure()` (CONFIRMED: `actor/add.rs` ensure=true creates-or-returns). Return the actor id.
- Add `whoami = "1"` to `crates/swissarmyhammer-kanban/Cargo.toml` (already a workspace dep in `apps/kanban-app/Cargo.toml`).

### Agent-facing ops (under `crates/swissarmyhammer-kanban/src/comment/`)
Model module layout on `src/attachment/` (mod.rs + add.rs + get.rs + list.rs + update.rs + delete.rs) and the `#[operation(verb, noun)]` macro pattern.
1. `mod.rs` — declare submodules; re-export the five command structs, `build_comment_member`, `resolve_comment_author`, and a `comment_member_to_json` helper.
2. `add.rs` — `AddComment { task_id, actor: Option<String>, text }`, `#[operation(verb="add", noun="comment")]`. Reads the task, resolves author via `resolve_comment_author`, appends `build_comment_member(...)`, writes the task back with `task.set("comments", ...)`. Returns the new member JSON. MUST preserve existing members.
3. `list.rs` — `ListComments { task_id }`, `#[operation(verb="list", noun="comments")]`. Returns the comments array sorted by timestamp ascending.
4. `get.rs` — `GetComment { task_id, id }` → one member or `KanbanError::CommentNotFound { id }` (variant ALREADY EXISTS in error.rs).
5. `update.rs` — `UpdateComment { task_id, id, text }` edits only `text` (who/when immutable). CommentNotFound if absent.
6. `delete.rs` — `DeleteComment { task_id, id }` removes by id. CommentNotFound if absent.
7. Wire `pub mod comment;` into `src/lib.rs` (next to `attachment`).

Note: the MCP agent always dispatches with an explicit actor (`claude-code`), so the OS-user fallback only triggers for actor-less callers — it never mis-attributes agent comments.

## Acceptance Criteria
- [ ] Five ops exist with correct `#[operation]` verb/noun annotations; `AddComment.actor` is `Option<String>`.
- [ ] `build_comment_member` and `resolve_comment_author` are `pub(crate)` and reusable; `ensure_os_user_actor` lives in the actor module.
- [ ] A member stores who/what/when + a stable ulid id; adding preserves existing members.
- [ ] explicit existing actor attributed; explicit non-existent actor errors; `actor: None` resolves+ensures the OS-user actor and succeeds.
- [ ] get/update/delete on a missing id return `KanbanError::CommentNotFound`; update changes text only.
- [ ] No `comment` Entity is ever created (assert no comment file under the tasks dir).
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] `add.rs` tests (modeled on `attachment/add.rs`): add two comments (explicit actor), re-read, assert 2 members w/ actor/text/timestamp/id and that the second add preserved the first.
- [ ] Explicit unknown actor errors; `actor: None` path writes a member whose `actor` is the ensured OS-user id and that actor entity now exists (idempotent on repeat).
- [ ] get/update/delete happy paths + CommentNotFound for a bogus id; update leaves actor/timestamp unchanged.
- [ ] Assert no standalone comment entity file is written.
- [ ] `cargo nextest run -p swissarmyhammer-kanban comment` — green.

## Workflow
- Use `/tdd` — write add+preserve, author-resolution paths, and CommentNotFound tests first, then implement.