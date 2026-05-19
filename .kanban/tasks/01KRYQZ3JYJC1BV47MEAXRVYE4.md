---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Remove claude-agent [lib] test = false quarantine and fix the broken lib unit tests
---
Discovered during the ACP finish run (not part of the 16 ACP cards — pre-existing debt).

## Problem
`crates/claude-agent/Cargo.toml` has `[lib]` `test = false` with the comment "Disable lib cfg(test) modules - need ACP 0.9.0 fixes". Effect: claude-agent's ~703 `#[cfg(test)]` lib unit tests are NEVER compiled into a test binary and never run under `cargo nextest run --workspace`. Only claude-agent's integration suite (`tests/`) runs in CI. The canonical workspace test runner is therefore green while silently not exercising claude-agent's lib unit tests.

When run explicitly (`cargo test -p claude-agent --lib`), ~38 of those lib unit tests fail **deterministically** (not flaky) — in modules such as `terminal_manager`, `path_validator`, `capability_validation`, `tools`, and `session` — and there was at least one hanging test. They are genuinely broken, quarantined behind `test = false`.

This predates the `acp` branch. The "ACP 0.9.0" comment is stale — the workspace is on `agent-client-protocol` 0.11 / schema 0.12.

## Target
- Investigate each broken claude-agent lib unit test; fix or delete each (a broken test is either fixed or, if it tests something no longer real, deleted — do not leave it quarantined).
- Remove the `[lib] test = false` line from `crates/claude-agent/Cargo.toml` so claude-agent's lib unit tests compile and run under `cargo nextest run --workspace` like every other crate.
- Note: card 11 deleted `crates/claude-agent/src/server.rs`, so the previously-reported hang in `server::tests::test_json_rpc_error_response_format` is likely already gone — re-baseline before assuming the failure set.
- Several ACP cards (6, 7, 10, 12, 15) added claude-agent lib `#[cfg(test)]` tests or were forced to route coverage through `tests/integration/` specifically because of this quarantine — once it is lifted, those lib tests will start running and must pass.

## Verify
- `crates/claude-agent/Cargo.toml` no longer disables lib tests.
- `cargo nextest run --workspace` runs claude-agent's lib unit tests and is fully green.