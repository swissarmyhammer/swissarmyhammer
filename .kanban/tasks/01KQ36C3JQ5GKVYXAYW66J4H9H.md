---
assignees:
- claude-code
depends_on:
- 01KQD0GV6NJATPBQX6SRH58V8S
- 01KQD0N8Y24T4FVQJDCH80QPQE
- 01KQD0NWYCZESV97RS8097W175
- 01KQD0P3HF6MG8ZSRX3P8MZR19
- 01KQ36AGXFCJF4PEEK2TDN6YQK
- 01KQ36B70YMBZ64YWB2JNTFY2F
- 01KQG4WHX5DKS64CANMF5ZMTWB
- 01KQG4X15BJ4EQ8K763TH39TMJ
- 01KQG8NVFWPEVR9YF4PTVKHAXC
- 01KQG8P8M4FVH5JHJYNX2XBM6C
position_column: review
position_ordinal: '80'
project: acp-upgrade
title: 'Workspace-wide green: full nextest + clippy after ACP 0.11 upgrade'
---
## What

Final integration check across the workspace after every per-crate task lands.

Steps:
1. `cargo update -p agent-client-protocol` (re-confirm `Cargo.lock` at 0.11.1).
2. `cargo build --workspace --all-targets` — must succeed, no warnings.
3. `cargo clippy --workspace --all-targets -- -D warnings`.
4. `cargo nextest run --workspace` — every test green.
5. Smoke-run an end-to-end ACP exchange using the existing example: `cargo run --example acp_stdio -p llama-agent` driven by `acp-conformance` fixtures (or the equivalent claude-agent harness in `claude-agent/tests/integration`). Confirm initialize → new_session → prompt → notifications → stop produces the expected output.

## Acceptance Criteria
- [x] `cargo build --workspace --all-targets` succeeds with zero warnings.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` succeeds.
- [ ] `cargo nextest run --workspace` — all tests pass. **3 failures, routed to follow-up tasks (see Validation Run Results below).**
- [x] At least one end-to-end ACP exchange runs successfully (driven via the integration test harness, not manual). Document which test exercised this in the task comments.

## Validation Run Results (acp/0.11-rewrite)

### Step 1: Cargo.lock at 0.11.1 — PASS
- `cargo update -p agent-client-protocol`: 0 packages updated.
- `Cargo.lock` already at `agent-client-protocol v0.11.1`.

### Step 2: `cargo build --workspace --all-targets` — PASS
- Zero warnings, zero errors.
- `Finished `dev` profile [unoptimized + debuginfo] target(s) in 59.80s`

### Step 3: `cargo clippy --workspace --all-targets -- -D warnings` — PASS
- Zero warnings, zero errors.
- `Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.33s`

### Step 4: `cargo nextest run --workspace` — 3 FAILURES out of 13197 tests

```
Summary [ 130.189s] 13197 tests run: 13194 passed (30 slow), 3 failed, 5 skipped
```

Failures (per workflow rule, NOT fixed here — routed back):
1. `avp-common context::tests::test_recording_is_always_on_with_no_env_vars` — recordings dir not created on Drop. Routed to **01KQG8NVFWPEVR9YF4PTVKHAXC**.
2. `avp-common context::tests::test_set_session_id_propagates_through_eager_with_agent` — same recording-flush regression. Routed to **01KQG8NVFWPEVR9YF4PTVKHAXC**.
3. `llama-agent integration::tool_use_multi_turn::test_validator_shaped_multi_turn_with_real_model` — model emits direct verdict instead of dispatching `read_file` tool. Routed to **01KQG8P8M4FVH5JHJYNX2XBM6C**.

### Step 5: End-to-end ACP exchange — PASS

Re-ran the four ACP integration test suites that drive a full prompt turn under the new 0.11 stack:

```
cargo nextest run -p llama-agent --test agent_tests \
    'integration::tool_call_round_trip' \
    'integration::acp_read_file' \
    'integration::acp_write_file' \
    'integration::acp_slash_command'

Summary [ 10.159s] 22 tests run: 22 passed, 61 skipped
```

Notable end-to-end coverage that exercised the full ACP 0.11 surface:
- `llama-agent::agent_tests integration::tool_call_round_trip::test_tool_call_round_trip_with_real_model` — 1.911s, drives a full prompt turn against a real model with tool dispatch.
- `llama-agent::agent_tests integration::tool_call_round_trip_via_mcp::test_full_round_trip_with_mcp_fetched_tools_against_real_model` — 6.951s, full round-trip with MCP-fetched tool schemas.
- `llama-agent::agent_tests integration::acp_stdio_transport::stdio_tests::*` — 2 tests, validate stdio transport server creation + stream exposure.
- All 7 `acp_write_file` and 4 `acp_read_file` tests, plus 8 `acp_slash_command` tests — exercise the per-method ACP plumbing.

## Tests
- [x] The end-to-end check is one of the existing integration tests — `tool_call_round_trip::test_tool_call_round_trip_with_real_model` and `tool_call_round_trip_via_mcp::test_full_round_trip_with_mcp_fetched_tools_against_real_model` both passed under the 0.11 stack and exercise initialize → new_session → prompt → tool_call → tool_call_update → stop.

## Workflow
- Pure validation — no source edits should be needed here. If something fails, route the fix back to the relevant per-crate task (don't fix in this task).
- 3 failures encountered during step 4. Two follow-up tasks created and added as `depends_on`. This validation task remains in `review` and cannot be marked done until those land and a re-run shows zero failures.

## Depends on
- 01KQ7KP7HVASAD4V45AJ41P39W (atomic SDK rewrite).
- 01KQ36AGXFCJF4PEEK2TDN6YQK (acp-conformance).
- 01KQ36B70YMBZ64YWB2JNTFY2F (consumers).
- 01KQG8NVFWPEVR9YF4PTVKHAXC (avp-common context recording test fix — route-back).
- 01KQG8P8M4FVH5JHJYNX2XBM6C (llama-agent multi-turn validator test fix — route-back).