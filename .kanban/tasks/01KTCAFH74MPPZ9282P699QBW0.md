---
assignees:
- claude-code
depends_on:
- 01KTCAEXMCAWWTE7FBGP7BE86Z
position_column: todo
position_ordinal: '8480'
project: card-comments
title: Route comment ops through dispatch and register in schema (agent-facing)
---
## What
Wire the comment command structs into the kanban **op** dispatch + schema so the MCP agent can call `{"op": "add comment", ...}`, `list comments`, `get comment`, `update comment`, `delete comment` end to end. (This is the agent-facing path. The UI does NOT use these ops — it uses the generic `entity.update_field` field-set path, handled in a separate task.) The `Noun::Comment`/`Comments` enum entries and `is_valid` (verb,noun) pairs ALREADY EXIST in `crates/swissarmyhammer-kanban/src/types/operation.rs` (lines ~96-97, ~144-145, ~281-283) — do NOT re-add them; this task connects them to the implementations.

NOTE: No `comment.yaml` command file and no `register_commands` changes are needed. CONFIRMED via research that the UI mutates fields through the generic `entity.update_field` command (editors are pure UI); comments use that path, so there is no `comment.add`/`update`/`delete` UI command. This task is purely the kanban-op (agent) surface.

Files:
1. `crates/swissarmyhammer-kanban/src/dispatch.rs`:
   - Add `async fn execute_comment_operation(processor, ctx, op)` modeled on `execute_attachment_operation` (~line 849). Map `Verb::Add → AddComment`, `Verb::List → ListComments`, `Verb::Get → GetComment`, `Verb::Update → UpdateComment`, `Verb::Delete → DeleteComment`. Use the same `req`/`req_task_id`/`op.get_string` helpers.
   - **Author: pass-through, NOT resolved here.** Build `AddComment { task_id, actor: <explicit `actor` param if present, else op.actor (the dispatching actor)>, text }`. Author resolution lives in `AddComment::execute` (dependency task) — dispatch only forwards the Option.
   - Add the match arm to `execute_operation`'s `match op.noun` (~line 928): `Noun::Comment | Noun::Comments => execute_comment_operation(&processor, ctx, op).await,` — currently it falls through to the `_ => unsupported operation` arm (dispatch.rs:942).
2. `crates/swissarmyhammer-kanban/src/schema.rs`:
   - Import the five comment structs and add them to the `KANBAN_OPERATIONS` static list (in a `// Comment` group, mirroring the `// Attachment` group at ~line 67).
   - Add an `add comment` example to `generate_kanban_examples()`.

## Acceptance Criteria
- [ ] `execute_operation` routes `Noun::Comment | Noun::Comments` to the comment handler (no longer "unsupported operation").
- [ ] All five comment ops appear in the generated MCP schema `op` enum and in `x-operation-schemas`.
- [ ] An `add comment` example appears in the schema examples.
- [ ] `add comment` dispatched with a dispatching actor records that actor; dispatched with none still succeeds (fallback handled in `AddComment::execute`).
- [ ] No `comment.yaml` command file is created and `register_commands` is unchanged.
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] In `dispatch.rs` test module: parse + execute `{"op":"add comment","task_id":...,"text":"hi"}` then `{"op":"list comments","task_id":...}` returns the member. Round-trip through `parse_input` → `execute_operation`.
- [ ] In `schema.rs` test module: extend the `test_kanban_operations_returns_full_list`-style assertions to require all five comment ops in the op enum, and `x-operation-schemas` count matches ops.
- [ ] Dispatch with an explicit dispatching actor attributes that actor on the resulting member.
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the dispatch round-trip + schema-enum assertions first.