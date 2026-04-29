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
position_column: todo
position_ordinal: fc80
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
- [ ] `cargo build --workspace --all-targets` succeeds with zero warnings.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` succeeds.
- [ ] `cargo nextest run --workspace` — all tests pass.
- [ ] At least one end-to-end ACP exchange runs successfully (driven via the integration test harness, not manual). Document which test exercised this in the task comments.

## Tests
- [ ] The end-to-end check is one of the existing integration tests — pick one that already drives a full prompt turn (e.g. `llama-agent/tests/integration/acp_stdio_transport.rs` or a `claude-agent/tests/integration/*.rs` test). Run it explicitly and capture the result.

## Workflow
- Pure validation — no source edits should be needed here. If something fails, route the fix back to the relevant per-crate task (don't fix in this task).

## Depends on
- 01KQ7KP7HVASAD4V45AJ41P39W (atomic SDK rewrite).
- 01KQ36AGXFCJF4PEEK2TDN6YQK (acp-conformance).
- 01KQ36B70YMBZ64YWB2JNTFY2F (consumers).