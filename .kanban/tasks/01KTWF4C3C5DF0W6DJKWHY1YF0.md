---
assignees:
- claude-code
position_column: todo
position_ordinal: '9880'
project: local-review
title: 'refactor(tests): extract the scripted ACP agent harness shared by fleet/verify/drive/tools-review tests'
---
## What

The scripted-agent test scaffolding (`ScriptedAgent`, `ScriptedAdapter::connect_to`, `dispatch`, `prompt_text`, `findings_json`, `confirm_json`) was near-verbatim duplicated across five files. **Update 2026-06-12 (task 01KTY91Y7AJRPJNBCVTV59HCJJ review pass):** the three validators-crate copies are CONSOLIDATED — one shared `ScriptedAgent` harness now lives in `crates/swissarmyhammer-validators/src/review/test_support.rs`, parameterized by `ScriptedAgentConfig` (`ForkMode` with `Unsupported` default, `default_response`, broadcast/bridge emit policy, `demand_permission`, `read_file`), together with the shared `prompt_text`, `with_pool`, `findings_json`, and `verdict_json` helpers. `fleet.rs`, `verify.rs`, `drive.rs` (including the former `FsReadingAgent`), and the pool tests' former `ForkExtAgent` all consume it.

Remaining scope — the tools-crate copies:

- `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`
- `crates/swissarmyhammer-tools/tests/integration/review_fixture.rs`

## Fix (remaining)

Have the two tools-crate copies consume the one shared harness instead of carrying their own. Options, in preference order:

1. Expose the validators harness behind a `test-support` feature (or `#[cfg(any(test, feature = "test-support"))]`) so swissarmyhammer-tools can dev-depend on it — mirrors how other crates expose test utils (e.g. agent-client-protocol-extras' `test-support` feature).
2. Or move the generic scripted-agent pieces into `acp_conformance::test_utils` (the shared `MockAgent`/`MockAgentAdapter` harness already lives there) and have the validators test_support re-export/wrap it.

Either way: delete the two tools-crate copies; do NOT create another variant.

## Why deferred

Cross-crate consumption needs a public test-support surface decision for the validators crate (its `review::test_support` is currently `#[cfg(test)]` + `pub(crate)`), which is beyond the scope of the fleet-orchestration review pass that consolidated the in-crate copies.

## Acceptance

- One scripted-agent harness, no per-file copies of `ScriptedAgent`/`dispatch`/`prompt_text` anywhere in the workspace.
- `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools -p acp-conformance` green.
- `cargo clippy --all-targets -- -D warnings` clean on touched crates.