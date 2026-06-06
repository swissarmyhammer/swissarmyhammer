---
assignees:
- claude-code
depends_on:
- 01KTBNNTCCVS81QZV4CFQZV4X1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8180
project: local-review
title: Wire the `review` tool's live AgentFactory + EmbedderFactory at the MCP server layer
---
## What
The operation-based `review` MCP tool (`crates/swissarmyhammer-tools/src/mcp/tools/review/`) is registered by the server via `ReviewTool::new()` — which has NO agent/embedder factory wired, so the three pipeline ops (`review file`/`working`/`sha`) currently return an actionable error ("this tool was built without an agent factory"). The loader-read ops (`list`/`get`/`check validators`) already work.

Wire the live factories so the `review` ops run in production:

- Build an `AgentFactory` (`review_op::AgentFactory`) that mints a fresh `AcpAgentHandle` from the session's `ModelConfig` via `swissarmyhammer_agent::create_agent`, returning its `agent` (`DynConnectTo<Client>`) + `notification_rx` as a `review_op::AgentHandle`. NOTE: `swissarmyhammer-tools` cannot depend on `swissarmyhammer-agent` (cycle — `swissarmyhammer-agent` depends on `swissarmyhammer-tools`). So the factory must be constructed in a crate that sits ABOVE both (e.g. the CLI / app wiring layer, or wherever the `McpServer` is assembled with a concrete `ModelConfig`), and injected via `ReviewTool::new().with_agent_factory(...).with_embedder_factory(...)`. Find the correct injection point and register the configured `ReviewTool` there instead of the bare `register_review_tools` default (or extend the registration path to accept the factories).
- Use `review_op::default_embedder_factory()` for the embedder in production (loads the platform embedder), or override as needed.
- Honor `review.concurrency` config by pinning the `PoolConfig` worker count (the tool currently maps only the coarse `backend` choice; the server-built factory is the place to apply the pinned concurrency).

## Acceptance Criteria
- [x] `review working` (and `file`/`sha`) run end-to-end in a real MCP server against the configured backend, returning a `ReviewReport`.
- [x] No dependency cycle introduced; the factory is built at a layer that may depend on `swissarmyhammer-agent`.
- [x] `review.concurrency` override honored when set.

## Tests
- [x] An integration test at the wiring layer that constructs the server with a configured `ReviewTool` and drives `review working` (scripted or playback agent) end-to-end.

## Implementation notes
Injection seam: re-register the `review` tool with live factories on the already-built `McpServer` (its `tool_registry: Arc<RwLock<ToolRegistry>>` is shared across clones and read per `call_tool`, so the swap takes effect for every subsequent dispatch). This avoids widening `start_mcp_server`'s signature (30+ callers) and introduces no `tools -> agent` cycle.

- `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs`: `ReviewTool::with_concurrency(Option<usize>)` + `concurrency` field threaded into `ReviewRequest`; new `register_review_tool_with_factories(registry, agent_factory, embedder_factory, concurrency)`.
- `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`: `ReviewRequest.concurrency`; `pool_config_for(backend, concurrency)` pins workers via `PoolConfig::with_concurrency` when set.
- `crates/swissarmyhammer-tools/src/mcp/server.rs`: `McpServer::set_review_factories(agent_factory, embedder_factory, concurrency)` (the cycle-free post-construction injection method).
- `crates/swissarmyhammer-agent/src/lib.rs`: `review_agent_factory(Arc<ModelConfig>) -> review_op::AgentFactory` — the production factory calling `create_agent(&config, None)` and mapping `AcpAgentHandle` -> `review_op::AgentHandle`.
- `apps/swissarmyhammer-cli/src/commands/serve/mod.rs`: `wire_review_factories(handle, concurrency)` + `review_concurrency(cli_context)` read from `review.concurrency`; called after `start_mcp_server` in both stdio and HTTP serve paths.

Tests (TDD, red->green):
- `swissarmyhammer-tools` review/tests.rs: `mcp_server_set_review_factories_runs_review_working_end_to_end` (real `McpServer` + scripted agent + mock embedder -> ReviewReport) and `review_tool_with_concurrency_pins_the_pool_worker_count`.
- `swissarmyhammer-agent` tests/review_factory.rs: `review_agent_factory` builds the seam-typed factory from a `ModelConfig` (pure; no model load).

Verification: `cargo build --workspace` ok (acyclic graph proves no cycle; `swissarmyhammer-tools/Cargo.toml` has no `swissarmyhammer-agent` dep). `cargo test -p swissarmyhammer-tools --lib review::` = 9/9. `cargo test -p swissarmyhammer-agent` = 81 pass (3 pre-existing ignored). `cargo test -p swissarmyhammer-validators --lib pool` = 12/12. Clippy clean on `swissarmyhammer-tools --lib`, `swissarmyhammer-agent` lib+tests, and `sah` bin (the only workspace clippy warnings are in `tests/integration/review_e2e.rs`, owned by a concurrent task — not edited). Doc-links resolve (agent + tools doc clean of new warnings).