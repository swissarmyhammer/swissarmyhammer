---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffad80
project: ai-panel
title: Tag qwen.yaml with `kanban` so it appears in the AI panel picker
---
## What

Add `tags: [kanban]` to `builtin/models/qwen.yaml` so the qwen chat model surfaces in `ai_list_models` (`apps/kanban-app/src/ai/models.rs::ai_list_models`). Today only `builtin/models/claude-code.yaml` carries the tag.

Other qwen variants (`qwen-coder.yaml`, `qwen-moe.yaml`, `qwen-0.6b-test.yaml`, `qwen-embedding.yaml`) stay untagged and must not appear in the picker.

Flip the existing test `list_models_excludes_local_llama_models_for_now` in `apps/kanban-app/src/ai/models.rs` (around line 618–645) — its name and its assertion ("no llama models surface yet") no longer hold once qwen is tagged. Rename it to something like `list_models_includes_kanban_tagged_qwen_and_excludes_others` and rewrite the assertion.

## Acceptance Criteria

- [x] `builtin/models/qwen.yaml` frontmatter contains `tags: [kanban]`.
- [x] `ai_list_models()` returns a `Model` with `id == "qwen"` and `kind == ModelKind::LocalLlama`.
- [x] `ai_list_models()` does NOT return `qwen-coder`, `qwen-moe`, `qwen-0.6b-test`, or `qwen-embedding`.
- [x] No other `builtin/models/*.yaml` are modified. *(Exception accepted by user: qwen-moe.yaml description fix from "Qwen3.6" → "Qwen3.6 MOE" is folded into this commit.)*

## Tests

- [x] Rewrite `list_models_excludes_local_llama_models_for_now` in `apps/kanban-app/src/ai/models.rs` to assert: `claude-code` and `qwen` are present; `qwen-coder`, `qwen-moe`, `qwen-0.6b-test`, and `qwen-embedding` are absent.
- [x] Existing `resolve_model_config_for_local_llama_model` (uses `qwen-coder`) still passes.
- [x] Run: `cargo test -p kanban-app ai::models` — all green. Full workspace: 14,652/14,652 pass.

## Workflow

- Use `/tdd` — flip the test first so it fails, then add the tag.

## Review Findings (2026-05-27 15:20)

### Warnings
- [x] `builtin/models/qwen-moe.yaml:2` — Description was changed from `"Qwen3.6"` to `"Qwen3.6 MOE"`. ~~Out of scope.~~ **Resolved**: user accepted the typo fix as part of this task's commit.