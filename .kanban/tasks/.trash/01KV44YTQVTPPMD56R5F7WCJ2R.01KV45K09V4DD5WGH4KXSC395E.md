---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv4581z1bgq9p2wzbww93y7n
  text: 'Reviewed against the actual code paths — this task MISDIAGNOSES the review-haiku root cause. Item #1 (`server.rs:610` `template_context.get_agent_config(None)` fallback) is labeled "the actual bug" but is NOT: that branch only fires on a `ModelManager::resolve_agent_config` ERROR and feeds the GLOBAL agent; fixing it leaves review still non-haiku. The real trigger is item #2''s territory: `review_model_config` reads `template_context.get("model")`, which `set_default_variables()`/`set_model_variable()` always populates with the literal `"claude"` default in production (via `load_for_cli`). So `review_agent_name_from(None, Some("claude")) = "claude"` → `find_agent_by_name("claude")` NotFound → None → global plain `claude-code`. The `claude-code-haiku` fallback is unreachable from the serve path. The existing green test `test_review_model_config_defaults_to_haiku_when_unset` masks this because its fixture skips `set_default_variables()`. Corrected, focused task: fj5x25m (01KV457MAQ2M8RB30C4FJ5X25M). Recommend superseding this task by fj5x25m and demoting #1/#3/#4 to optional hygiene, not "the bug".'
  timestamp: 2026-06-14T22:50:10.785357+00:00
position_column: todo
position_ordinal: a080
project: claude-model-select
title: Unify model selection on ONE ModelManager resolver; stop template-context from feeding agent spawn
---
## Problem
Model/agent selection is resolved in several places. Most go through the builtin model definitions (`find_agent_by_name` + `parse_model_config`), which is the ONLY path that carries a Claude model's CLI switches (`ClaudeCodeConfig.args`, e.g. `["--model","haiku"]` from `builtin/models/claude-code-haiku.yaml`). One path bypasses the builtins and returns a switch-less default `claude-code`, so the builtin's switches never reach the spawned `claude` process. Net effect: `qwen` (llama) works because a llama model carries its full config by name; Claude tiers (haiku) silently get dropped on the leaky path.

The spawn seam is already correct: `swissarmyhammer-agent/src/lib.rs` `create_agent_with_options` → `claude_agent_config_from_model` (lib.rs:861) → `claude_code_args` (lib.rs:371) → `build_claude_agent_config` sets `claude.extra_args` (lib.rs:944) → claude-agent `build_base_command` appends them. Any builtin-derived `ModelConfig` WILL deliver its switches. The bug is purely which resolver feeds this seam.

Template context is a separate concern (Liquid `{{ model }}` prompt variable, defaulting to the literal `"claude"` in `template_context.rs:665/670`) and must NOT participate in agent/model selection.

## Canonical resolver (the one true path)
`ModelManager` in `crates/swissarmyhammer-config/src/model.rs`:
- default scope: `resolve_agent_config` (model.rs:1833) → `get_agent` → `find_agent_by_name` + `parse_model_config`; unconfigured → `ModelConfig::claude_code()`.
- review scope: `resolve_review_agent_config` (model.rs:1931) via `resolve_review_agent_name`/`review_agent_name_from` (review.model → model: → `claude-code-haiku`).
- `--model` overrides funnel through the same `find_agent_by_name` lookup.
Every agent-spawn call site must obtain its `ModelConfig` from this family. No other source.

## What
### 1. Remove the leaky template-context fallback (the actual bug)
- `crates/swissarmyhammer-tools/src/mcp/server.rs` `resolve_agent_config` (~lines 576-610): DELETE the `template_context.get_agent_config(None)` fallback at ~line 610. On `ModelManager::resolve_agent_config` error, fall back to `ModelConfig::claude_code()` (explicit switch-less default) or propagate — never `template_context`.

### 2. Redirect review resolution to the canonical resolver (kill duplicate logic)
- `apps/swissarmyhammer-cli/src/commands/serve/mod.rs` `review_model_config` (~line 87): replace the hand-rolled `template_context.get("review.model")/get("model")` + `review_agent_name_from` with a direct `ModelManager::resolve_review_agent_config(&ModelPaths::sah())` (and `resolve_review_agent_name` for the log line). Stop reading `template_context` for model selection entirely. Update `wire_review_factories` doc accordingly.
- Update the serve unit tests (`commands::serve::tests`) that currently inject `review.model`/`model` via `template_context` to instead write a real config (mirror the model.rs tests: `ensure_config_structure` + `std::fs::write` of `model:` / `review:\n  model:`), since resolution now reads config files. The "unknown name" test should expect the global-default fallback (`resolve_agent_config`), not `None`.

### 3. Remove (or quarantine) the ModelConfig-from-template-context family
- `crates/swissarmyhammer-config/src/template_context.rs`: once server.rs:610 no longer calls it, `get_agent_config` / `get_all_agent_configs` / `try_get_config` / `try_extract_config` have no production callers. Remove them, OR if the structured `agent.default` / `agent.configs.<wf>` workflow-config blob feature must survive, re-express it as builtin model NAMES resolved through `ModelManager` (not inline `ModelConfig` blobs). Confirm no other caller first.
- KEEP `resolve_model_name`/`set_model_variable` (the `{{ model }}` = "claude" Liquid var) but document them as prompt-rendering ONLY, never agent selection.

### 4. De-duplicate the kanban-app resolver (optional cleanup, not a bug)
- `apps/swissarmyhammer-cli/src/commands/agent/acp.rs` `resolve_model_config` (~line 219) is builtin-backed but is a 3rd copy of the `claude-code shortcut + find_agent_by_name + parse_model_config` pattern. Extract a shared `ModelManager::resolve_by_id(name)` helper and route this + the `--model` override path through it.

## Acceptance Criteria
- [ ] No production code resolves an agent/spawn `ModelConfig` from `template_context`; grep shows `get_agent_config` has no production callers (removed or quarantined).
- [ ] `sah serve` default agent and review agent both resolve via `ModelManager`; a configured `model: <x>` or `review.model: <x>` delivers that builtin's `ClaudeCodeConfig.args` to the spawned `claude` (provable via the spawn argv log).
- [ ] Unconfigured review scope spawns claude with `--model haiku`; unconfigured default scope spawns plain `claude-code` (no `--model`); `model: qwen` drives both default and review to the llama executor.
- [ ] Template `{{ model }}` rendering still works and is documented as rendering-only.

## Tests
- [ ] Real-path test: an unconfigured review run through the wired `ReviewTool` produces a SpawnConfig whose `extra_args == ["--model","haiku"]` (already exists from prior task — keep it green).
- [ ] New: `McpServer::resolve_agent_config` with a config error does NOT yield a template-context default; assert it returns `claude-code` (empty args) or errors, never a fabricated config.
- [ ] New: serve `review_model_config` tests rewritten to write real config files; cover unset→haiku, `model: qwen-0.6b-test`→llama (inherited by review), `review.model: claude-code` overrides overall, unknown name→global default.
- [ ] Regression: a test proving `template_context` is NOT consulted for model selection (e.g. setting `template_context` model to a bogus value does not change the resolved agent ModelConfig).
- [ ] Run: `cargo test -p swissarmyhammer-config -p swissarmyhammer-tools -p swissarmyhammer-agent -p swissarmyhammer-cli` and `cargo clippy ... -- -D warnings` — all green.

## Notes
- Working tree currently has one uncommitted experimental edit in `crates/claude-agent/src/claude_process.rs::execute_spawn` (an argv log at the spawn) — keep it; it is the observability proof for this work.
- See project memory `review-model-resolution` and `real-path-tests-not-mocks`.

## Workflow
- Use `/tdd` — write the failing real-path/regression tests first, then implement. #haiku