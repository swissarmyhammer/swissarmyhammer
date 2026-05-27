---
assignees:
- claude-code
depends_on:
- 01KSMZ1Y2CWKJE32PG16T328K7
position_column: todo
position_ordinal: '8580'
project: ai-panel
title: 'Integration test: per-board model isolation (two boards, two models, no bleed)'
---
## What

A single integration test that demonstrates the per-board model contract end-to-end: two separate boards each remember their own model, and switching between them does not leak state.

This is automated, not manual — no "open the app" steps.

### Approach

Write a Rust integration test (not a unit test inside `board/update.rs`) so it exercises the full disk round-trip in two independent `.kanban` directories.

Location: `crates/swissarmyhammer-kanban/tests/per_board_model_isolation.rs` (new file). Pattern after any existing integration test in `crates/swissarmyhammer-kanban/tests/`.

### Steps

1. Create two `TempDir`s, each with its own `.kanban` directory and its own `KanbanContext`.
2. `InitBoard::new("A").execute(&ctx_a)`; `InitBoard::new("B").execute(&ctx_b)`.
3. `UpdateBoard::new().with_model("claude-code").execute(&ctx_a)`.
4. `UpdateBoard::new().with_model("qwen").execute(&ctx_b)`.
5. Read each `.kanban/boards/board.yaml` from disk as raw text; assert:
   - A's file contains `model: claude-code`.
   - B's file contains `model: qwen`.
   - A's file does NOT contain `qwen`.
   - B's file does NOT contain `claude-code`.
6. Call `GetBoard::default().execute()` on each context; assert `result["model"]` round-trips its own id.

### Optional follow-on (only if it fits cleanly — otherwise skip)

If `swissarmyhammer-config::model::ModelManager::find_agent_by_name` + `parse_model_config` are callable from a test, also assert:

- For A's model id → `ModelConfig::executor_type() == ModelExecutorType::ClaudeCode`.
- For B's model id → `ModelConfig::executor_type() == ModelExecutorType::LlamaAgent`.

This proves the stored id resolves to a runnable executor, not just an opaque string. If it pulls in heavy test setup, drop it — the round-trip assertions alone are enough.

## Acceptance Criteria

- [ ] New test file `crates/swissarmyhammer-kanban/tests/per_board_model_isolation.rs` exists.
- [ ] The test passes locally.
- [ ] The test fails if a regression makes `UpdateBoard` write the model to a shared/global location (e.g. a static or a single shared file).

## Tests

- [ ] `test_per_board_model_isolation` — the test described above.
- [ ] Run: `cargo test -p swissarmyhammer-kanban --test per_board_model_isolation`.

## Workflow

- Use `/tdd` — write the test first; with tasks 1 and 2 already done, it should pass immediately. The point is the regression guard, not the red→green dance.