---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
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
- [ ] `SpawnConfig` has an `extra_args: Vec<String>` field that is appended verbatim to the spawned `claude` command in `build_base_command`.
- [ ] When `extra_args` contains `--model`, ephemeral mode does not also append a second `--model` flag.
- [ ] `ClaudeConfig.extra_args` flows into both `SpawnConfig` construction sites in `agent.rs`.
- [ ] Existing behavior with empty `extra_args` is unchanged (no extra flags appear).

## Tests
- [ ] In `crates/claude-agent/src/claude_process.rs` tests: a test that builds a `SpawnConfig` with `extra_args: ["--model", "haiku"]` and asserts `build_base_command` produces a command whose args contain `--model haiku` in order. (Use the existing pattern for inspecting the built `Command` args; if none exists, assert on a `Vec<String>` of args extracted from the command.)
- [ ] A test that with `ephemeral: true` AND `extra_args: ["--model", "haiku"]`, the command contains exactly one `--model` occurrence.
- [ ] A test that with `ephemeral: true` and empty `extra_args`, the command still contains the ephemeral `--model haiku` and `--no-session-persistence` (regression guard for current behavior).
- [ ] A test that with empty `extra_args` and `ephemeral: false`, no `--model` flag is present.
- [ ] Run: `cargo test -p claude-agent` — all green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.