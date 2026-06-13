---
assignees:
- claude-code
position_column: todo
position_ordinal: 9c80
project: local-review
title: 'refactor(llama-agent): decompose oversized ACP server functions and consolidate real-model test helpers'
---
## What

Follow-up for substantive review findings swept into 01KTYAYGGC6HBQDN74BXS2Y7FG's review from the llama-agent companion changes (the cheap ones — StateDirGuard hoist, shared `test_agent_config`/`test_model_config`/`test_cwd` helpers, `SEEDED_STATE_BYTES`, batch-size literals — were fixed there). These are real but each is a sizable refactor of llama-agent internals:

- [ ] `crates/llama-agent/src/acp/server.rs` — `new_session` is ~155-220 lines mixing transport validation, system-prompt injection, agent-tools mount, external MCP client assembly, tool discovery/routing, session registration, and mode-state assembly, each with its own failure policy. Extract `assemble_session_mcp_clients` and `discover_and_register_tools` helpers (continuing the `register_session` extraction), and add a rustdoc comment on `new_session` summarizing the lifecycle.
- [ ] `crates/llama-agent/src/acp/server.rs` — `ext_method` is ~200 lines inlining eleven route handlers. Group arms into per-domain helpers (`route_fs`, `route_terminal`, `route_session_fork`); the six identical lock→call→map_err terminal arms are the natural first extraction.
- [ ] `crates/llama-agent/src/acp/server.rs` — `require_capability` takes two adjacent `impl Into<String>` messages distinguishable only by position; group into a small `CapabilityErrorMessages { undeclared, uninitialized }` struct (or enum-keyed closure) so a swap can't compile.
- [ ] `crates/llama-agent/src/acp/session_fork.rs` — `extension_error(session_id, kind, ...)` takes two adjacent `&str`s; make the kind a dedicated newtype/enum mapping to the shared contract constants. NOTE: do this in lockstep with claude-agent's now-canonical `acp_error::session_error` (same shape) — do not let the two backends diverge.
- [ ] `crates/llama-agent/tests/integration/real_model_helpers.rs` — make `real_model_config` the single canonical Qwen test-model `AgentConfig` constructor: point the six sibling copies (`agent_generate_path.rs`, `agent_cache_integration.rs`, `tool_call_round_trip.rs`, `streaming_generation.rs`, `incremental_processing.rs`, `mtp_streaming.rs`) at it, parameterizing real variation axes; replace `try_init_agent`-style broad skip heuristics (skips on `loadingfailed`) with the shared `is_environmental_model_failure`/`build_real_model_server` path so model-loading regressions panic everywhere.
- [ ] `crates/llama-agent/tests/integration/real_model_helpers.rs` — name the config literals: `TEST_MODEL_BATCH_SIZE` (64), the model-download retry policy constants, and `TEST_MODEL_THREADS` (4), each with a one-line rationale.
- [ ] `crates/llama-agent/tests/integration/real_model_helpers.rs` — `text_prompt` duplicates `acp/server.rs::tests::hook_lifecycle::text_prompt`; move one copy into `llama_agent::acp::test_utils` and use it from both sites.

## Acceptance Criteria

- [ ] `new_session` and `ext_method` each read as a flat sequence of named phases under ~50 lines of actual code.
- [ ] One canonical real-model `AgentConfig` constructor; `loadingfailed` panics (not skips) in every real-model test.
- [ ] `cargo test -p llama-agent` and `cargo clippy -p llama-agent --all-targets -- -D warnings` green.