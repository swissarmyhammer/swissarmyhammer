---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffac80
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

1. Expose the validators harness behind a `test-support` feature (or `#[cfg(any(test, feature = \"test-support\"))]`) so swissarmyhammer-tools can dev-depend on it — mirrors how other crates expose test utils (e.g. agent-client-protocol-extras' `test-support` feature).
2. Or move the generic scripted-agent pieces into `acp_conformance::test_utils` (the shared `MockAgent`/`MockAgentAdapter` harness already lives there) and have the validators test_support re-export/wrap it.

Either way: delete the two tools-crate copies; do NOT create another variant.

## Why deferred

Cross-crate consumption needs a public test-support surface decision for the validators crate (its `review::test_support` is currently `#[cfg(test)]` + `pub(crate)`), which is beyond the scope of the fleet-orchestration review pass that consolidated the in-crate copies.

## Acceptance

- One scripted-agent harness, no per-file copies of `ScriptedAgent`/`dispatch`/`prompt_text` anywhere in the workspace.
- `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools -p acp-conformance` green.
- `cargo clippy --all-targets -- -D warnings` clean on touched crates.

## Review Findings (2026-06-14 16:48)

Scope: `review file` over this task's changed files (test_support.rs, mod.rs, tools tests.rs, review_fixture.rs, both Cargo.toml). Default session backend. Core acceptance verified directly: exactly ONE `struct ScriptedAgent` / `fn dispatch` / `fn prompt_text` (all in test_support.rs); old per-file variants `FsReadingAgent`/`ForkExtAgent` fully removed; `test-support` feature wired in validators Cargo.toml and consumed by the tools crate's dev-dependency, mirroring the agent-client-protocol-extras / acp-conformance pattern. No blockers, no warnings — 3 nits.

### Nits
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs` — `ScriptedAgentConfig` is public (re-exported behind `test-support`) but derived neither `Debug` nor `Clone`, while every field is both. Added `#[derive(Debug, Clone)]` (all fields, incl. `broadcast::Sender`, satisfy both).
- [x] `crates/swissarmyhammer-validators/src/review/test_support.rs` — `rebind_broadcast` reconstructed `ScriptedAgentConfig` field-by-field only to override `broadcast`/`bridge_to_connection`. Collapsed to `ScriptedAgentConfig { broadcast: Some(broadcast), bridge_to_connection, ..base.config.clone() }` via the new `Clone` derive.
- [x] `crates/swissarmyhammer-tools/tests/integration/review_fixture.rs` — `pub fn scripted_factory` was documented only generically; rewrote its `///` to state it adapts a scripted agent into an `AgentFactory`, opening a fresh `broadcast(256)` notification channel per connection.

### Verification (2026-06-14)
- `cargo clippy -p swissarmyhammer-validators --features test-support --all-targets` — clean, exit 0.
- `cargo test -p swissarmyhammer-validators review` — 121 passed, 0 failed.
- `cargo test -p swissarmyhammer-tools --test tools_tests review` (review_e2e) — 4 passed, 0 failed.
- `cargo test -p swissarmyhammer-tools --test review_global_subscriber` — 1 passed, 0 failed.