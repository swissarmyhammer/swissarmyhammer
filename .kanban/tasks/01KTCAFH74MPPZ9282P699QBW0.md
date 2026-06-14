---
assignees:
- claude-code
depends_on:
- 01KTCAEXMCAWWTE7FBGP7BE86Z
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa580
project: card-comments
title: Route comment ops through dispatch and register in schema (agent-facing)
---
## What
Wire the comment command structs into the kanban **op** dispatch + schema so the MCP agent can call `{"op": "add comment", ...}`, `list comments`, `get comment`, `update comment`, `delete comment` end to end. (This is the agent-facing path. The UI does NOT use these ops — it uses the generic `entity.update_field` field-set path, handled in a separate task.) The `Noun::Comment`/`Comments` enum entries and `is_valid` (verb,noun) pairs ALREADY EXIST in `crates/swissarmyhammer-kanban/src/types/operation.rs` (lines ~96-97, ~144-145, ~281-283) — do NOT re-add them; this task connects them to the implementations.

NOTE: No `comment.yaml` command file and no `register_commands` changes are needed. CONFIRMED via research that the UI mutates fields through the generic `entity.update_field` command (editors are pure UI); comments use that path, so there is no `comment.add`/`update`/`delete` UI command. This task is purely the kanban-op (agent) surface.

SCHEMA SURFACES (the full-vs-wire split is MERGED — see `crates/swissarmyhammer-kanban/src/schema.rs`): `generate_kanban_mcp_schema` is now the slim WIRE surface (`generate_mcp_schema_wire`: `op` enum + compact `x-op-signatures` required-param map; drops `WIRE_DROPPED_KEYS` = x-operation-schemas/x-operation-groups/x-forgiving-input/examples), and `generate_kanban_mcp_schema_full` is the CLI-facing FULL surface (keeps all the heavy keys). Registering the five ops in `KANBAN_OPERATIONS` automatically lands them on BOTH surfaces — but tests must assert against the right surface (see below).

RESPONSE SHAPES (op-token-diet, landed): comment mutations return the `task_mutation_ack` envelope with top-level `id` = TASK id (`add comment` additionally carries the new member under `comment`); see the dependency card ^p7be86z. Dispatch passes responses through unchanged.

Files:
1. `crates/swissarmyhammer-kanban/src/dispatch.rs`:
   - Add `async fn execute_comment_operation(processor, ctx, op)` modeled on `execute_attachment_operation` (~line 849). Map `Verb::Add → AddComment`, `Verb::List → ListComments`, `Verb::Get → GetComment`, `Verb::Update → UpdateComment`, `Verb::Delete → DeleteComment`. Use the same `req`/`req_task_id`/`op.get_string` helpers.
   - **Author: pass-through, NOT resolved here.** Build `AddComment { task_id, actor: <explicit `actor` param if present, else op.actor (the dispatching actor)>, text }`. Author resolution lives in `AddComment::execute` (dependency task) — dispatch only forwards the Option.
   - Add the match arm to `execute_operation`'s `match op.noun` (~line 928): `Noun::Comment | Noun::Comments => execute_comment_operation(&processor, ctx, op).await,` — currently it falls through to the `_ => unsupported operation` arm (dispatch.rs:942).
2. `crates/swissarmyhammer-kanban/src/schema.rs`:
   - Import the five comment structs and add them to the `KANBAN_OPERATIONS` static list (in a `// Comment` group, mirroring the `// Attachment` group).
   - Add an `add comment` example to `generate_kanban_examples()` (examples are FULL/CLI-surface only — the wire drops them by contract; do not assert examples on the wire schema).
3. `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs`:
   - Extend `is_task_modifying_operation` to include `(Add, Comment)`, `(Update, Comment)`, `(Delete, Comment)` so comment mutations attach `_plan` with `_plan._meta.affected_task_id` populated from the ack's top-level `id` (the task id). This mirrors the Tag/Untag fix landed by op-token-diet card ^5jxh97r — without it, comment mutations are silently missed by the `_plan` extraction exactly like tag/assign were.

IMPLEMENTATION NOTE (discovered during TDD): `parse::normalize_params` globally aliased `task_id`/`taskId` → `id`, which silently DROPPED the owning-task reference for member ops (both keys meaningful: `task_id` = task, `id` = member). Fixed by making the alias noun-aware in `crates/swissarmyhammer-kanban/src/parse/mod.rs` — the `task_id → id` alias is skipped for member nouns (Comment/Comments/Attachment/Attachments; camelCase still snake_cases to `task_id`). This also fixes the same latent bug for attachment ops dispatched via `parse_input`. Regression test: `parse::tests::test_member_ops_keep_task_id_distinct_from_id`.

## Acceptance Criteria
- [x] `execute_operation` routes `Noun::Comment | Noun::Comments` to the comment handler (no longer "unsupported operation"). (The match is now exhaustive — the unreachable `_` fallback arm was removed, so future nouns are a compile error.)
- [x] All five comment ops appear in the WIRE schema (`generate_kanban_mcp_schema`): in `properties.op.enum` AND as keys in `x-op-signatures` with the right required params (e.g. `"add comment": ["task_id", "text"]` — `actor` is optional so it must NOT appear in the signature).
- [x] All five comment ops appear in the FULL schema's `x-operation-schemas` (`generate_kanban_mcp_schema_full`).
- [x] An `add comment` example appears in the FULL schema's examples.
- [x] `add comment` dispatched with a dispatching actor records that actor; dispatched with none still succeeds (fallback handled in `AddComment::execute`).
- [x] `add comment` / `update comment` / `delete comment` dispatched through the MCP kanban tool attach `_plan` with `affected_task_id` = the task id.
- [x] No `comment.yaml` command file is created and `register_commands` is unchanged.
- [x] `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-tools -- -D warnings` clean.

## Tests
- [x] In `dispatch.rs` test module: parse + execute `{"op":"add comment","task_id":...,"text":"hi"}` then `{"op":"list comments","task_id":...}` returns the member. Round-trip through `parse_input` → `execute_operation`. (`dispatch_add_comment_then_list_round_trip`; plus `dispatch_comment_get_update_delete_round_trip` covering the other three arms.)
- [x] In `schema.rs` test module: extend the existing wire/full surface tests (`test_wire_schema_structure_omits_heavy_keys`, `test_full_schema_structure_keeps_heavy_keys`, the `x-op-signatures` tests) to require the five comment ops in the wire `op` enum + `x-op-signatures` (with `add comment` requiring exactly `task_id`,`text`), and the full schema's `x-operation-schemas` count to match the op count. (`test_schema_includes_comment_ops`, `test_full_schema_has_comment_example`; the count is asserted by the existing generic `test_full_kanban_schema_operation_schemas_count`.)
- [x] Dispatch with an explicit dispatching actor attributes that actor on the resulting member. (`dispatch_add_comment_attributes_dispatching_actor`)
- [x] In `crates/swissarmyhammer-tools/src/mcp/tools/kanban/mod.rs` tests: `test_add_comment_plan_carries_affected_task_id` modeled on `test_tag_task_plan_carries_affected_task_id` (from ^5jxh97r) — `add comment` response carries `_plan._meta.affected_task_id` equal to the commented task's id.
- [x] `cargo nextest run -p swissarmyhammer-kanban` and the tools kanban-filtered run — green. (1492/1492 and 69/69.)

## Workflow
- Use `/tdd` — write the dispatch round-trip + wire/full schema assertions + the `_plan` regression test first. (Done: all six new tests watched RED, then GREEN.)