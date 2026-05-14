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
position_column: done
position_ordinal: ffffffffffffffffffffffffb880
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
- [x] `cargo nextest run --workspace` — all tests pass.
- [x] At least one end-to-end ACP exchange runs successfully (driven via the integration test harness, not manual). Documented in step 5 below.

## Validation Run Results (acp/0.11-rewrite) — Re-run after route-back fixes (2026-04-29)

Both follow-up tasks landed and are done:
- 01KQG8NVFWPEVR9YF4PTVKHAXC (avp-common context recording) — DONE
- 01KQG8P8M4FVH5JHJYNX2XBM6C (llama-agent multi-turn validator test) — DONE

Re-running every step on `acp/0.11-rewrite`:

### Step 1: Cargo.lock at 0.11.1 — PASS
- `cargo update -p agent-client-protocol`: `Locking 0 packages to latest compatible versions`.
- `Cargo.lock` confirmed at `agent-client-protocol v0.11.1`.

### Step 2: `cargo build --workspace --all-targets` — PASS
- Zero warnings, zero errors.
- `Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.31s`.

### Step 3: `cargo clippy --workspace --all-targets -- -D warnings` — PASS
- Zero warnings, zero errors.
- `Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.22s`.

### Step 4: `cargo nextest run --workspace` — PASS

```
Summary [ 163.352s] 13196 tests run: 13196 passed (29 slow, 1 leaky), 6 skipped
```

Zero failures. Both previously-failing avp-common tests and the llama-agent validator test now run cleanly (the validator test is correctly `#[ignore]`d under default invocation per its acceptance criteria).

### Step 5: End-to-end ACP exchange — PASS

```
cargo nextest run -p llama-agent --test agent_tests \
    'integration::tool_call_round_trip' \
    'integration::acp_read_file' \
    'integration::acp_write_file' \
    'integration::acp_slash_command'

Summary [  13.523s] 22 tests run: 22 passed, 61 skipped
```

Notable end-to-end coverage that exercised the full ACP 0.11 surface:
- `llama-agent::agent_tests integration::tool_call_round_trip::test_tool_call_round_trip_with_real_model` — 3.161s, drives a full prompt turn against a real model with tool dispatch.
- `llama-agent::agent_tests integration::tool_call_round_trip_via_mcp::test_full_round_trip_with_mcp_fetched_tools_against_real_model` — 7.770s, full round-trip with MCP-fetched tool schemas.
- All 7 `acp_write_file` and 4 `acp_read_file` tests, plus 8 `acp_slash_command` tests — exercise the per-method ACP plumbing (initialize → new_session → prompt → notifications → stop).

## Tests
- [x] The end-to-end check is one of the existing integration tests — `tool_call_round_trip::test_tool_call_round_trip_with_real_model` and `tool_call_round_trip_via_mcp::test_full_round_trip_with_mcp_fetched_tools_against_real_model` both passed under the 0.11 stack and exercise initialize → new_session → prompt → tool_call → tool_call_update → stop.

## Workflow
- Pure validation — no source edits made here. The 3 failures from the first run were routed to follow-up tasks; both are now done and the re-run confirms the workspace is fully green.

## Depends on
- 01KQ7KP7HVASAD4V45AJ41P39W (atomic SDK rewrite).
- 01KQ36AGXFCJF4PEEK2TDN6YQK (acp-conformance).
- 01KQ36B70YMBZ64YWB2JNTFY2F (consumers).
- 01KQG8NVFWPEVR9YF4PTVKHAXC (avp-common context recording test fix — DONE).
- 01KQG8P8M4FVH5JHJYNX2XBM6C (llama-agent multi-turn validator test fix — DONE).