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

SCHEMA SURFACES (the full-vs-wire split is MERGED — see `crates/swissarmyhammer-kanban/src/schema.rs`): `generate_kanban_mcp_schema` is now the slim WIRE surface (`generate_mcp_schema_wire`: `op` enum + compact `x-op-signatures` required-param map; drops `WIRE_DROPPED_KEYS` = x-operation-schemas/x-operation-groups/x-forgiving-input/examples), and `generate_kanban_mcp_schema_full` is the CLI-facing FULL surface (keeps all the heavy keys). Registering the five ops in `KANBAN_OPERATIONS` automatically lands them on BOTH surfaces — but tests must assert against the right surface (see below).

Files:
1. `crates/swissarmyhammer-kanban/src/dispatch.rs`:
   - Add `async fn execute_comment_operation(processor, ctx, op)` modeled on `execute_attachment_operation` (~line 849). Map `Verb::Add → AddComment`, `Verb::List → ListComments`, `Verb::Get → GetComment`, `Verb::Update → UpdateComment`, `Verb::Delete → DeleteComment`. Use the same `req`/`req_task_id`/`op.get_string` helpers.
   - **Author: pass-through, NOT resolved here.** Build `AddComment { task_id, actor: <explicit `actor` param if present, else op.actor (the dispatching actor)>, text }`. Author resolution lives in `AddComment::execute` (dependency task) — dispatch only forwards the Option.
   - Add the match arm to `execute_operation`'s `match op.noun` (~line 928): `Noun::Comment | Noun::Comments => execute_comment_operation(&processor, ctx, op).await,` — currently it falls through to the `_ => unsupported operation` arm (dispatch.rs:942).
2. `crates/swissarmyhammer-kanban/src/schema.rs`:
   - Import the five comment structs and add them to the `KANBAN_OPERATIONS` static list (in a `// Comment` group, mirroring the `// Attachment` group).
   - Add an `add comment` example to `generate_kanban_examples()` (examples are FULL/CLI-surface only — the wire drops them by contract; do not assert examples on the wire schema).

## Acceptance Criteria
- [ ] `execute_operation` routes `Noun::Comment | Noun::Comments` to the comment handler (no longer "unsupported operation").
- [ ] All five comment ops appear in the WIRE schema (`generate_kanban_mcp_schema`): in `properties.op.enum` AND as keys in `x-op-signatures` with the right required params (e.g. `"add comment": ["task_id", "text"]` — `actor` is optional so it must NOT appear in the signature).
- [ ] All five comment ops appear in the FULL schema's `x-operation-schemas` (`generate_kanban_mcp_schema_full`).
- [ ] An `add comment` example appears in the FULL schema's examples.
- [ ] `add comment` dispatched with a dispatching actor records that actor; dispatched with none still succeeds (fallback handled in `AddComment::execute`).
- [ ] No `comment.yaml` command file is created and `register_commands` is unchanged.
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] In `dispatch.rs` test module: parse + execute `{"op":"add comment","task_id":...,"text":"hi"}` then `{"op":"list comments","task_id":...}` returns the member. Round-trip through `parse_input` → `execute_operation`.
- [ ] In `schema.rs` test module: extend the existing wire/full surface tests (`test_wire_schema_structure_omits_heavy_keys`, `test_full_schema_structure_keeps_heavy_keys`, the `x-op-signatures` tests) to require the five comment ops in the wire `op` enum + `x-op-signatures` (with `add comment` requiring exactly `task_id`,`text`), and the full schema's `x-operation-schemas` count to match the op count.
- [ ] Dispatch with an explicit dispatching actor attributes that actor on the resulting member.
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the dispatch round-trip + wire/full schema assertions first.