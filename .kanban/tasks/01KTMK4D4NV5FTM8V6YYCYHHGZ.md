---
assignees:
- claude-code
position_column: todo
position_ordinal: '9780'
project: local-review
title: 'Config: store & resolve a review-specific model'
---
## What
Add a per-purpose model selection to the model config layer so the `review` tool can use a different model than the global default. Storage lives in `<cwd>/.sah/sah.yaml` under the same `review:` table as `review.concurrency` (i.e. `review.model: <name>`), so it reads back through the established `template_context.get("review.model")` path.

All work is in `crates/swissarmyhammer-config/src/model.rs`:

- Introduce a `ModelTarget` enum (`Default`, `Review`).
- Generalize the write path: `update_config_with_agent` (currently hardcodes top-level `model:`, ~line 2010) takes a `ModelTarget`. `Default` keeps writing the top-level `model:` scalar. `Review` writes/updates `model` inside the `review:` sub-mapping, creating the `review:` mapping if absent and **preserving** any existing keys there (notably `concurrency`).
- Add `use_agent_for(agent_name, target, paths)`; keep `use_agent` as a thin wrapper delegating with `ModelTarget::Default` (so all existing callers and tests are untouched).
- Add `get_review_agent(paths) -> Result<Option<String>, ModelError>` mirroring `get_agent` (~line 1757) but reading `review.model` (nested), returning `None` when absent.
- Add `resolve_review_agent_config(paths) -> Result<ModelConfig, ModelError>`: if `get_review_agent` is `Some`, `find_agent_by_name` + `parse_model_config`; else fall back to `resolve_agent_config(paths)` (which itself falls back to the global `model:` then the `claude-code` default).

Security: `use_agent_for` must run the same `validate_agent_name_security` + `validate_agent` checks as `use_agent`.

## Acceptance Criteria
- [ ] `use_agent_for(name, ModelTarget::Review, paths)` writes `review.model` and leaves a pre-existing top-level `model:` and `review.concurrency` intact.
- [ ] `get_review_agent` returns the review model name when set, `None` otherwise.
- [ ] `resolve_review_agent_config` returns the review-specific model when set, else the global model, else `claude-code`.
- [ ] `use_agent` behavior and signature unchanged (delegates to `use_agent_for` with `Default`).
- [ ] Invalid/empty/unsafe names are rejected by `use_agent_for` exactly as by `use_agent`.

## Tests
- [ ] Unit test in `model.rs`: `use_agent_for` review target writes `review.model`, asserting the YAML still contains the prior `model:` and `review.concurrency` keys.
- [ ] Unit test: `resolve_review_agent_config` returns review model when set; returns global when only `model:` set; returns `claude-code` when neither set.
- [ ] Unit test: `get_review_agent` returns `None` on a fresh/empty config and `Some(name)` after a review write.
- [ ] Unit test: security validation rejects empty/`../`-bearing names via `use_agent_for`.
- [ ] `cargo test -p swissarmyhammer-config model` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.