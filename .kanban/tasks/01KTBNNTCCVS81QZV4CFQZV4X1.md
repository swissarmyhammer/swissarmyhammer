---
assignees:
- claude-code
depends_on:
- 01KTBNKCZ2JRRX514XWHPFB7V1
- 01KTBNM0YGVRJQJSCTQBDHR68H
- 01KTBNMJY54KG5K7BWG29C2J1J
- 01KTBNN3A9JNQ5VGD1JN16RCT8
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffc80
project: local-review
title: 'Operation-based `review` MCP tool: review file/working/sha + validator introspection'
---
## What
Expose the engine as an OPERATION-BASED MCP tool in `crates/swissarmyhammer-tools/src/mcp/tools/review/`, dispatched by an `op` field exactly like `git`, `kanban`, and `code_context` (single tool, verb-noun op-dispatch — NOT a tool-per-verb). Thin wrapper: parse op + args, build a scope, call the engine, return structured results.

**Engine entry point:** `run_review(scope, repo_path, loader, conn, embedder, pool, fleet_config, now) -> ReviewReport` already existed (stage-4). This task adds the thin connection/pool-choreography driver `run_review_over_agent(agent, notification_rx, scope, repo_path, loader, conn, embedder, pool_config, fleet_config, now)` in `swissarmyhammer-validators::review::drive` — it stands up the `Client.builder().connect_with(...)` connection, builds the `AgentPool`, and calls `run_review`. The MCP tool is a dispatch shim: map op→scope, resolve the connection + opts, call the driver, return the report.

## Acceptance Criteria
- [x] Engine driver exists (scope → pool → fan-out → guard → verify → drain → synthesize); the tool is a dispatch shim that calls it. Reuses existing `run_review`; adds `run_review_over_agent`.
- [x] The op-dispatched `review` tool is registered with `review file`/`review working`/`review sha` + `list/get/check validators`.
- [x] `backend` modifier honored (session→remote / local→single worker); `validators` accepted (subset is a later refinement). Op-dispatch/registration/structs mirror the git tool; no `install`/`dimensions`.
- [x] Connection + CWD resolved from the session/work-dir, never `current_dir()`.

## Tests
- [x] Real-pipeline integration test through the registered tool: temp git repo with a planted duplicate + seeded on-disk code_context index + scripted ACP agent; `review working` returns a report flagging the issue as a confirmed blocker. (`review sha`/`review file` share the same dispatch→driver path.)
- [x] `list validators` returns seeded user + project layers with correct source layers and `probes`; `get validator` returns rule bodies + probes; `check validators` errors on a validator declaring an unknown probe.
- [x] Tool registration test (the ops appear in the registry); `cargo test -p swissarmyhammer-tools review` green (7 passed), clippy clean.

## Notes / follow-ups
- The three `review` ops need a live agent + loaded embedder, supplied to the tool via constructor-injected `AgentFactory` / `EmbedderFactory` seams (DI, not global state). The default `ReviewTool::new()` registered by the server serves the loader-read ops immediately and returns an actionable error for the `review` ops until the server wires the backend factory — a small follow-up wiring task at the server layer. The `validators[]` subset filter is accepted but not yet applied (full matching set runs).

## Review Findings (2026-06-05 19:42)

Verdict: blocking. The tool's structure, op-dispatch, registration, validator-introspection ops, CWD/session resolution, and DI seam are all correct and match the git-tool pattern and ARCHITECTURE.md tiers (no dependency cycle: `swissarmyhammer-tools` and `swissarmyhammer-validators` depend on the lower-tier `claude-agent` + `agent-client-protocol`, never on the `swissarmyhammer-agent` facade, which is the crate that depends on `-tools`). Shipping the loader-read ops live with the three `review` ops returning an actionable error until factory `01KTCJ0T1X5QWFREA3JCADY19P` wires the backend is an acceptable MVP split. However, the driver itself — the headline deliverable of THIS task — has a production-only correctness bug that the scripted test cannot catch, and that the server-wiring follow-up would wire a real backend straight into.

Both suites pass and clippy is clean: `cargo test -p swissarmyhammer-validators --lib review` = 67 passed (incl. `review::drive::tests::review_working_drives_the_pipeline_over_a_scripted_agent`); `cargo test -p swissarmyhammer-tools review` = 7 passed; `cargo clippy -p swissarmyhammer-tools -p swissarmyhammer-validators --all-targets` = 0 warnings (files touched to force a real recompile).

### Blockers
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs` (`run_review_over_agent`) — **Double notification delivery in production.** RESOLVED. The driver now feeds the pool's notifier from EXACTLY ONE source: the agent's `notification_rx`, drained by a single `forward_notifications` task spawned by the new `build_pool_notifier` seam. The connection-side `on_receive_notification(...)` → notifier forwarder was removed entirely (fix option (a)). `notification_rx` is the authoritative half a real `AcpAgentHandle` exposes (a `resubscribe()` of the backend broadcast that `wrap_claude_into_handle` also bridges onto the connection), so collecting from it once matches production. The connection still carries the bridged re-emission; the driver simply no longer collects it a second time. Module docs now state the single-path invariant and why double-feeding corrupts the JSON.

### Warnings
- [x] `crates/swissarmyhammer-validators/src/review/drive.rs` tests / `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs` — RESOLVED. Both scripted agents were reshaped to the real-handle dual-emission shape: the agent streams its reply onto a backend `broadcast::Sender` whose `subscribe()` is fed in as `notification_rx`, AND bridges the same notification onto the live connection (mirroring `forward_session_notifications`). The reshaped `review_working_drives_the_pipeline_over_a_scripted_agent` and the tools-side `review_working_through_the_registered_tool_flags_a_planted_duplicate` now exercise the production dual-path. A new decisive driver test `notification_rx_is_the_pools_single_collected_stream` streams a multi-chunk reply through `build_pool_notifier` and asserts the collected text equals the single original byte-for-byte; it also reproduces the old dual-feed and asserts it doubles the collected length (corruption). Verified RED→GREEN: temporarily re-adding a second forwarder made the single-feed assertion fail with the exact interleaved/garbled JSON, then pass once removed.

### Nits
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs` (`execute`) — RESOLVED. The empty-`op` default now lives in one place: `op` is read, empty strings are filtered out, and the default `"review working"` is applied via a single `unwrap_or`. The `"review working" | ""` match arm was reduced to `"review working"`, so there is one source of truth for the default.