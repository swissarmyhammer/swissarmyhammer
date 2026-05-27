---
assignees:
- claude-code
depends_on:
- 01KSMZ15RVAEQ6MYQEPGXSEP9K
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffae80
project: ai-panel
title: Persist selected model per-board via `model` field on the board entity
---
## What

Add a `model` field to the board entity, stored at `.kanban/boards/board.yaml`. The entity layer is schemaless (`Entity` is `key → serde_json::Value`; see `swissarmyhammer-entity/src/io.rs`), so storage requires no new types — just an extra `entity.set("model", json!(...))` in `UpdateBoard` and an extra field in `GetBoard`'s JSON.

### Files

- `crates/swissarmyhammer-kanban/src/board/update.rs`
  - Extend `UpdateBoard` struct with `pub model: Option<String>`.
  - Add builder method `with_model(mut self, model: impl Into<String>) -> Self`.
  - In `execute`, after the existing `name`/`description` writes: if `self.model.is_some()`, validate the id, then `entity.set("model", json!(model))`.
  - Validation: `claude-code` is always valid. Any other id must resolve via `swissarmyhammer_config::model::ModelManager::find_agent_by_name(id)` AND parse to `ModelConfig` whose `executor_type()` is `ClaudeCode` or `LlamaAgent` (embedding executors are not chat agents). Mirror the logic already in `apps/kanban-app/src/ai/models.rs::resolve_model_config` — extract a shared validator if it makes the code cleaner, otherwise inline.
  - On invalid id, return `KanbanError` (pick the closest existing variant — likely `InvalidArgument` or similar; check `crates/swissarmyhammer-kanban/src/error.rs`).
  - Include `model` in the `Ok(json!({…}))` return so the response shape matches `GetBoard`.

- `crates/swissarmyhammer-kanban/src/board/get.rs`
  - Add `let board_model = board.get_str("model");` next to the existing `board_name` / `board_description` reads.
  - Include `"model": board_model` in BOTH JSON outputs: the `include_counts: false` branch (~line 68–74) AND the `include_counts: true` branch (~line 145–159).
  - `null` when unset.

### Why not store in `board.yaml` directly via a fresh loader?

The board entity already round-trips through `EntityContext::read`/`write` to `.kanban/boards/board.yaml`. Adding a key to that entity is the minimal, architecture-respecting change. No new file, no new struct, no new serializer.

## Acceptance Criteria

- [x] `UpdateBoard::new().with_model("qwen").execute(&ctx)` succeeds and `.kanban/boards/board.yaml` on disk contains a `model: qwen` line.
- [x] `UpdateBoard::new().with_model("claude-code").execute(&ctx)` succeeds.
- [x] `UpdateBoard::new().with_model("bogus-xyz").execute(&ctx)` returns `ExecutionResult::Failed` with an error mentioning the unknown id.
- [x] `UpdateBoard::new().with_model("qwen-embedding").execute(&ctx)` returns `Failed` (embedding executor cannot back a chat agent).
- [x] `GetBoard::default().execute(&ctx)` returns `"model"` in both `include_counts: true` and `include_counts: false` shapes; value is `null` when unset, the chosen id when set.
- [x] `UpdateBoard` with only `name` set does not clobber an existing `model` value.
- [x] Existing `test_update_board_name` and `test_update_board_description` still pass.

## Tests

Add to `crates/swissarmyhammer-kanban/src/board/update.rs::tests`:

- [x] `test_update_board_model_persists_to_yaml` — set model, then read the raw `.kanban/boards/board.yaml` file from disk and assert it contains the model id.
- [x] `test_update_board_model_round_trips_via_get_board` — set model, then `GetBoard`, assert `result["model"] == "qwen"`.
- [x] `test_update_board_rejects_unknown_model` — `with_model("bogus-xyz")` → `Failed`.
- [x] `test_update_board_rejects_embedding_model` — `with_model("qwen-embedding")` → `Failed`.
- [x] `test_update_board_accepts_claude_code` — round-trips `claude-code`.
- [x] `test_update_board_accepts_qwen` — round-trips `qwen` (depends on prior task tagging it).
- [x] `test_update_board_model_preserved_when_only_name_changes` — set model, then update name only, then GetBoard, assert model still present.

Add to `crates/swissarmyhammer-kanban/src/board/get.rs::tests`:

- [x] `test_get_board_model_null_when_unset` — fresh board has `result["model"]` as null (both `include_counts: true` and `false`).

Run: `cargo test -p swissarmyhammer-kanban board::update board::get`.

## Workflow

- Use `/tdd` — write all the failing tests first, then make them pass.

## Implementation Notes

- Added `swissarmyhammer-config` workspace dependency to `crates/swissarmyhammer-kanban/Cargo.toml` so the validator can call `ModelManager::find_agent_by_name` and `parse_model_config`. No new dependency direction — config does not depend on kanban.
- Extracted a private `validate_model_id` helper at module scope in `update.rs` rather than inlining inside `execute` — it mirrors `resolve_model_config` in `kanban-app/src/ai/models.rs` and the kept-separate function keeps `execute` short and the validation rules in one place. Uses the existing `KanbanError::InvalidValue` variant with field `"model"` for both unknown ids and embedding-executor rejections.
- Also threaded `model` through `dispatch.rs::execute_board_operation` so MCP / CLI callers of `update board` can pass it. The dispatch reads `op.get_string("model")` next to the existing `name` / `description` reads.
- All 64 `board::*` tests pass (9 new + 55 existing). `cargo check --workspace --lib --tests` and `cargo clippy -p swissarmyhammer-kanban -- -D warnings` both clean.