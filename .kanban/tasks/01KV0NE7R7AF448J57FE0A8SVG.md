---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa880
project: claude-model-select
title: Make claude-code-haiku the baked-in default for the review scope
---
## What
Change the review scope so that when nothing is configured, it falls back to `claude-code-haiku` instead of the global default model. The fallback must be effective in the production wiring, not just a library helper.

Two resolution paths exist (confirm both during implementation):
- `crates/swissarmyhammer-config/src/model.rs`: `resolve_review_agent_config` (~line 1862) reads `review.model` (via `get_review_agent` ~line 1831) and falls back to the global default. Change the fallback chain to: `review.model` → `claude-code-haiku` builtin → global default. Add/extend a constructor like `ModelConfig::claude_code()` (there is one ~line for the default) with a `claude_code_haiku()` equivalent that loads the builtin by name, or resolve `claude-code-haiku` through `ModelManager::find_agent_by_name` + `parse_model_config`.
- `apps/swissarmyhammer-cli/src/commands/serve/mod.rs`: `review_model_config` (~line 72) currently returns `None` when `review.model` is unset, and `wire_review_factories` (~line 45) then falls back to the global `agent_config`. Update so an unset `review.model` resolves to `claude-code-haiku` (reuse the config-layer fallback rather than duplicating the name). An explicitly configured `review.model` still wins; an invalid name still warns and falls back.

Keep `claude-code-haiku` as a named constant in one place; do not scatter the string literal.

## Acceptance Criteria
- [x] With no `review.model` set, the review scope resolves to the `claude-code-haiku` model config.
- [x] An explicitly set `review.model` overrides the baked-in default.
- [x] An invalid `review.model` warns and falls back (to claude-code-haiku, the new review default).
- [x] The global/default scope (non-review) is unchanged — still `claude-code`.
- [x] The `claude-code-haiku` name exists as a single shared constant.

## Tests
- [x] In `crates/swissarmyhammer-config/src/model.rs` tests: with empty config, `resolve_review_agent_config` returns a `ModelConfig` whose executor args are `["--model", "haiku"]` (i.e. claude-code-haiku).
- [x] A test that an explicit `review.model: claude-code` overrides the default and yields plain claude-code (no `--model haiku`).
- [x] A test that the default (non-review) scope is still claude-code with empty args.
- [x] In `apps/swissarmyhammer-cli` (serve module tests): `review_model_config` with unset `review.model` resolves to claude-code-haiku (not None / not the global default).
- [x] Run: `cargo test -p swissarmyhammer-config` and `cargo test -p swissarmyhammer-cli` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #haiku

## Implementation Notes
- Shared constant: `swissarmyhammer_config::model::REVIEW_DEFAULT_AGENT = "claude-code-haiku"` (also re-exported from crate root via `pub use model::{...}` not required; referenced through the `model` module). Single source of truth.
- New constructor: `ModelConfig::claude_code_haiku() -> Result<Self, ModelError>` resolves the builtin by `REVIEW_DEFAULT_AGENT` via `find_agent_by_name` + `parse_model_config`.
- `resolve_review_agent_config` chain: `review.model` → `claude_code_haiku()` → `resolve_agent_config` (only if the builtin cannot be resolved).
- serve `review_model_config`: unset OR invalid name both fall back to `claude_code_haiku()` via new helper `review_default_config()`; explicit valid name still wins.