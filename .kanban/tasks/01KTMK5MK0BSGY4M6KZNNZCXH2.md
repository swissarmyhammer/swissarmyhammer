---
assignees:
- claude-code
position_column: todo
position_ordinal: 9a80
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