---
assignees:
- claude-code
position_column: todo
position_ordinal: '9880'
project: local-review
title: 'refactor(tests): extract the scripted ACP agent harness shared by fleet/verify/drive/tools-review tests'
---
## What

The scripted-agent test scaffolding (`ScriptedAgent`, `ScriptedAdapter::connect_to`, `dispatch`, `prompt_text`, `findings_json`, `confirm_json`) is near-verbatim duplicated across five files:

- `crates/swissarmyhammer-validators/src/review/drive.rs` (tests)
- `crates/swissarmyhammer-validators/src/review/fleet.rs` (tests)
- `crates/swissarmyhammer-validators/src/review/verify.rs` (tests)
- `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`
- `crates/swissarmyhammer-tools/.../review_fixture.rs` (wherever the fixture copy lives — locate with `grep -rn "struct ScriptedAgent"`)

The copies have already drifted: fleet.rs `response_for` returns `Option<String>` (with error injection via `None`), drive.rs returns `String`; fleet.rs `findings_json` wraps in prose, drive.rs does not; drive.rs adds `demand_permission` / `bridge_to_connection` variants and an `FsReadingAgent`.

## Fix

Extract the scripted ACP agent harness once — into `acp_conformance::test_utils` (preferred: the shared `MockAgent`/`MockAgentAdapter` harness already lives there, and drive.rs's `LateAnsweringAgent` demonstrates the migration pattern) — parameterizing:

- the response-script lookup (substring → response, with optional per-entry error)
- whether replies are published to a backend broadcast, the live connection, or both
- optional mid-turn agent→client round-trips (permission demand, fs read)

Then have all five files consume it and delete the copies. `numbered_session_response` (already in `acp_conformance::test_utils`) covers the session-allocation piece.

## Why deferred

Flagged as a warning in the review of task 01KTW52WB3Q5KMNAWWJNYDRM59 (the copies predate that change). Not done inline because it is a five-file migration across two crates with divergent behavioral variants that each need a parameterization decision — well beyond the scope of wiring `TolerantResponseRouter`.

## Acceptance

- One scripted-agent harness, no per-file copies of `ScriptedAgent`/`dispatch`/`prompt_text`.
- `cargo test -p swissarmyhammer-validators -p swissarmyhammer-tools -p acp-conformance` green.
- `cargo clippy --all-targets -- -D warnings` clean on touched crates.