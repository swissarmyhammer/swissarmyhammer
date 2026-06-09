---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8a80
project: local-review
title: 'E2E: review runs over ACP against a real local model'
---
## What
Prove the `review` pipeline really runs over ACP against a **real local model** (not a scripted agent), end-to-end. This is the verification half of the work and is independent of the CLI/config plumbing — it exercises the production `review_agent_factory` directly against the `qwen-0.6b-test` builtin (a local llama-agent GGUF chat model).

Reference pattern: `apps/kanban-app/tests/ai_panel_e2e.rs` already drives `qwen-0.6b-test` over a real in-process ACP agent (see `resolve_qwen_test_config`, GPU gating, and the long op budget). Mirror its model-resolution and gating, but instead build the review `AgentHandle` from `swissarmyhammer_agent::review_agent_factory(Arc::new(qwen_config))` and run the review pipeline.

Likely home: a new integration test under `crates/swissarmyhammer-tools/tests/integration/` (or alongside `crates/swissarmyhammer-validators/src/review/tests.rs` if it can reach `swissarmyhammer-agent` without a dep cycle — prefer the tools integration dir, mirroring `mcp_server_set_review_factories_runs_review_working_end_to_end`).

Steps in the test:
- Resolve `qwen-0.6b-test` → `ModelConfig` via `ModelManager::find_agent_by_name` + `parse_model_config`.
- Build the production factory and run `review` over a tiny fixture (a small temp git repo or a single in-repo file scope) using the same call path the MCP `review` op uses.
- Assert the run **completes** and returns a well-formed `ReviewReport` (the `markdown`/`counts` are present and parse; no error). Assert **structure, not finding content** — a 0.6B model will not reliably produce real findings, so do NOT assert specific bugs are found.

Gating: GPU-gate exactly like the existing llama-agent e2e tests (per project convention the CI Test runner has a Metal GPU; coverage runs force CPU and skip GPU-only tests). Use a generous op budget (the kanban e2e uses 240s).

## Acceptance Criteria
- [ ] A new e2e test drives the real `review` pipeline against `qwen-0.6b-test` over the in-process ACP connection produced by `review_agent_factory`.
- [ ] The test asserts the pipeline completes and yields a well-formed `ReviewReport` (counts present, markdown non-error), without asserting specific finding content.
- [ ] The test is gated to the GPU runner using the same mechanism as the other llama-agent e2e tests and is skipped under the coverage/CPU gate.
- [ ] On the GPU runner the test passes; the model download is cached like other llama tests.

## Tests
- [ ] The new e2e test itself is the deliverable; it must pass on the GPU runner.
- [ ] Confirm it is correctly skipped (not failed) in a CPU-only run.

## Workflow
- Use `/tdd` — write the failing e2e first (it fails because nothing drives review over a real local model), then make it pass.

## Review Findings (2026-06-08 16:05)

### Warnings
- [x] `crates/swissarmyhammer-agent/tests/review_real_model_e2e.rs:218` — The `assert_eq!(counts.blockers + counts.warnings + counts.nits, counts.confirmed, ...)` asserts strict equality, but the two sides are counted at different stages of `synthesize`: `counts.confirmed` is `verified.iter().filter(|v| v.confirmed).count()` (pre-dedup, synthesize.rs:75), while the per-severity tallies are `section.len()` over the set returned by `dedup_exact` (synthesize.rs:79, 106-108). `dedup_exact` exists precisely to collapse exact-duplicate confirmed findings, so whenever the 0.6B model emits two identical confirmed findings (same file/line/validator/rule/claim — plausible for a small stochastic model on a tiny diff) the deduped sum is strictly less than `confirmed` and the assertion fails. It passed once on the GPU runner, but that does not prove the invariant always holds for a nondeterministic model — this is a latent flake. Assert the true invariant instead: `counts.blockers + counts.warnings + counts.nits <= counts.confirmed` (kept findings never exceed confirmed). The header-presence assertion already covers "well-formed, non-error markdown".

### Nits
- [x] `crates/swissarmyhammer-agent/Cargo.toml:35` — `model-embedding` is added as a dev-dependency but neither test in the crate (`review_real_model_e2e.rs`, `review_factory.rs`) references `model_embedding::` — the embedder is obtained opaquely via `default_embedder_factory()`. Rust does not warn on unused dev-deps, so it is harmless, but it is dead config; drop it unless a follow-up test needs it.