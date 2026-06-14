---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa680
project: claude-model-select
title: Plumb extra CLI args through the claude-agent spawn path
---
## What
The `claude-code` executor currently has no way to pass `--model` (or any switch) to the `claude` CLI. The only model selection that exists is a hardcoded `--model haiku` inside `configure_ephemeral_mode`. This task adds a generic "extra args" passthrough to the spawn path in the `claude-agent` crate so a caller can supply arbitrary CLI switches.

Files:
- `crates/claude-agent/src/claude_process.rs`
  - Add `extra_args: Vec<String>` to `SpawnConfig` (struct around line 110).
  - In `build_base_command` (around line 486), after the base command is built, append `extra_args` to the command.
  - Decide precedence vs `configure_ephemeral_mode` (around line 544), which hardcodes `--model haiku --no-session-persistence`. Rule: if `extra_args` already contains a `--model`, ephemeral mode must NOT also append its own `--model` (avoid a duplicate/conflicting `--model` on the command line); ephemeral's `--no-session-persistence` is unaffected. Implement a small helper that detects whether `--model` is already present in `extra_args`.
- `crates/claude-agent/src/config.rs`
  - Add `extra_args: Vec<String>` (defaulted empty) to `ClaudeConfig` (struct around line 92) so the value can be carried from config into the spawn step. Do not change the existing `model: String` field behavior.
- `crates/claude-agent/src/agent.rs`
  - At the two `SpawnConfig` construction sites (around lines 1501 and 1807), populate `extra_args` from the `ClaudeConfig`.

## Acceptance Criteria
- [x] `SpawnConfig` has an `extra_args: Vec<String>` field that is appended verbatim to the spawned `claude` command in `build_base_command`.
- [x] When `extra_args` contains `--model`, ephemeral mode does not also append a second `--model` flag.
- [x] `ClaudeConfig.extra_args` flows into both `SpawnConfig` construction sites in `agent.rs`.
- [x] Existing behavior with empty `extra_args` is unchanged (no extra flags appear).

## Tests
- [x] In `crates/claude-agent/src/claude_process.rs` tests: a test that builds a `SpawnConfig` with `extra_args: ["--model", "haiku"]` and asserts `build_base_command` produces a command whose args contain `--model haiku` in order. (Use the existing pattern for inspecting the built `Command` args; if none exists, assert on a `Vec<String>` of args extracted from the command.)
- [x] A test that with `ephemeral: true` AND `extra_args: ["--model", "haiku"]`, the command contains exactly one `--model` occurrence.
- [x] A test that with `ephemeral: true` and empty `extra_args`, the command still contains the ephemeral `--model haiku` and `--no-session-persistence` (regression guard for current behavior).
- [x] A test that with empty `extra_args` and `ephemeral: false`, no `--model` flag is present.
- [x] Run: `cargo test -p claude-agent` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #haiku

## Implementation Notes
Changed (this task's surgical scope — the `model.rs`/`builtin/models/claude-code-haiku.yaml` in the working tree belong to sibling task 01KV0NDKZQ1C3KZYHXXZX71GQ2, already in review):
- `crates/claude-agent/src/claude_process.rs`: added `extra_args: Vec<String>` (`#[builder(default)]`) to `SpawnConfig`; `build_base_command` gained an `extra_args: &[String]` param and appends them verbatim via `command.args(extra_args)`; added helper `extra_args_have_model`; `configure_ephemeral_mode` only adds its own `--model haiku` when no caller `--model` is present (`--no-session-persistence` always added in ephemeral mode). Updated production + existing test call sites; added 3 new tests (the 4th criterion — ephemeral+empty regression guard — is covered by the existing `test_ephemeral_mode_disables_cli_session_persistence`).
- `crates/claude-agent/src/config.rs`: added `extra_args: Vec<String>` (`#[serde(default)]`) to `ClaudeConfig` and to the `Default for AgentConfig` literal.
- `crates/claude-agent/src/agent.rs`: both `SpawnConfig::builder()` sites now `.extra_args(self.config.claude.extra_args.clone())`.

Verification (fresh): `cargo test -p claude-agent` = 317 passed / 0 failed + 6 doc-tests passed; `cargo clippy -p claude-agent --all-targets -- -D warnings` = exit 0, 0 warnings; `cargo fmt -p claude-agent -- --check` = exit 0.