---
assignees:
- claude-code
depends_on:
- 01KTMK4D4NV5FTM8V6YYCYHHGZ
position_column: todo
position_ordinal: '9980'
project: local-review
title: Wire the review pool to the review-specific model
---
## What
Make the live `review` agent factory use the review-specific model when one is configured, falling back to the global resolved `agent_config` otherwise. Today `wire_review_factories` (`apps/swissarmyhammer-cli/src/commands/serve/mod.rs:38`) always builds the factory from `server.tool_context.agent_config` (the single global default).

Files: `apps/swissarmyhammer-cli/src/commands/serve/mod.rs`
- Add `review_model(cli_context) -> Option<String>` mirroring `review_concurrency` (~line 53): read `template_context.get("review.model")` as a string.
- In `wire_review_factories`, if a review model name is set, resolve it to an `Arc<ModelConfig>` via `ModelManager::find_agent_by_name` + `parse_model_config` and build `review_agent_factory` from it; on resolution failure, log a warning and fall back to `server.tool_context.agent_config`. When unset, keep current behavior (global `agent_config`).
- Thread the resolved override into `wire_review_factories` (add a parameter) and update the call site in the serve handler, which already computes `review_concurrency(cli_context)`.

Keep the cycle-free boundary intact: resolution uses `swissarmyhammer_config::ModelManager` (already a dependency) and the existing `swissarmyhammer_agent::review_agent_factory`.

## Acceptance Criteria
- [ ] With `review.model` set, the review pool's factory builds from that model's `ModelConfig` (its `executor_type()` matches the selected model).
- [ ] With `review.model` unset, the factory uses the global `agent_config` exactly as before (no behavior change).
- [ ] An unresolvable `review.model` logs a warning and falls back to the global config rather than failing to wire.

## Tests
- [ ] Test in the serve module: given a `CliContext`/template-context with `review.model` set to a known builtin (e.g. `qwen-0.6b-test`), the resolved override `ModelConfig.executor_type()` is the llama-agent executor; with it unset the resolved config equals the global default's executor (`claude-code`).
- [ ] Test: an unknown `review.model` resolves to the global fallback (no panic, warning path).
- [ ] `cargo test -p swissarmyhammer-cli serve` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.