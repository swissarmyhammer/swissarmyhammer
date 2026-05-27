---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
project: ai-panel
title: Tag qwen.yaml with `kanban` so it appears in the AI panel picker
---
## What

Add `tags: [kanban]` to `builtin/models/qwen.yaml` so the qwen chat model surfaces in `ai_list_models` (`apps/kanban-app/src/ai/models.rs::ai_list_models`). Today only `builtin/models/claude-code.yaml` carries the tag.

Other qwen variants (`qwen-coder.yaml`, `qwen-moe.yaml`, `qwen-0.6b-test.yaml`, `qwen-embedding.yaml`) stay untagged and must not appear in the picker.

Flip the existing test `list_models_excludes_local_llama_models_for_now` in `apps/kanban-app/src/ai/models.rs` (around line 618–645) — its name and its assertion ("no llama models surface yet") no longer hold once qwen is tagged. Rename it to something like `list_models_includes_kanban_tagged_qwen_and_excludes_others` and rewrite the assertion.

## Acceptance Criteria

- [ ] `builtin/models/qwen.yaml` frontmatter contains `tags: [kanban]`.
- [ ] `ai_list_models()` returns a `Model` with `id == "qwen"` and `kind == ModelKind::LocalLlama`.
- [ ] `ai_list_models()` does NOT return `qwen-coder`, `qwen-moe`, `qwen-0.6b-test`, or `qwen-embedding`.
- [ ] No other `builtin/models/*.yaml` are modified.

## Tests

- [ ] Rewrite `list_models_excludes_local_llama_models_for_now` in `apps/kanban-app/src/ai/models.rs` to assert: `claude-code` and `qwen` are present; `qwen-coder`, `qwen-moe`, `qwen-0.6b-test`, and `qwen-embedding` are absent.
- [ ] Existing `resolve_model_config_for_local_llama_model` (uses `qwen-coder`) must still pass — `resolve_model_config` does not gate on the `kanban` tag, only `ai_list_models` does. If that test was load-bearing for "is local llama plumbing alive at all", leave it alone.
- [ ] Run: `cargo test -p kanban-app ai::models` — all green.

## Workflow

- Use `/tdd` — flip the test first so it fails, then add the tag.