---
assignees:
- claude-code
depends_on:
- 01KZ23MX9AB1QX9E4Q26S0PY85
position_column: todo
position_ordinal: ec80
project: drop-llama-agent
title: Delete the llama-agent crate
---
## What

Delete the `llama-agent` crate now that nothing consumes it.

- Delete the `crates/llama-agent/` directory.
- Remove `crates/llama-agent` from the `members` list and remove the
  `llama-agent` entry from `[workspace.dependencies]` in the root
  `Cargo.toml`.
- Remove the `llama-agent` dependency from
  `crates/acp-conformance/Cargo.toml`, and delete or re-point any
  acp-conformance test target that drives llama-agent as the implementation
  under test. `acp-conformance` itself STAYS — it is a generic ACP suite; only
  two of its source files name llama at all (`ext_method.rs`,
  `prompt_turn.rs`, one or two lines each).
- Remove `llama-agent` from `apps/swissarmyhammer-cli/Cargo.toml`.
- Remove the `package(llama-agent)` override block from `.config/nextest.toml`.
- Regenerate `Cargo.lock` with `cargo check --workspace`.

DO NOT touch `crates/llama-common`, `crates/llama-embedding`,
`crates/model-loader`, `crates/ane-embedding`, `crates/model-embedding`, or
`crates/swissarmyhammer-embedding`. `llama-common` is a dependency of
`llama-embedding`, `model-loader`, and `ane-embedding`, and the embedding
stack stays.

### Subtasks

- [ ] Remove the dependency from acp-conformance and the CLI.
- [ ] Delete the crate directory and the workspace entries.
- [ ] Remove the nextest override.
- [ ] Regenerate `Cargo.lock`.

## Acceptance Criteria

- [ ] `crates/llama-agent/` does not exist.
- [ ] `grep -rn "llama-agent" --include=Cargo.toml --include=*.toml .` returns
      nothing outside `target/`.
- [ ] `crates/llama-common/`, `crates/llama-embedding/`,
      `crates/model-loader/`, and `crates/ane-embedding/` are unchanged.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0 with
      zero warnings.

## Tests

- [ ] Run `cargo nextest run --workspace` — the whole suite passes and no
      `llama-agent` test binary is listed.
- [ ] Run `cargo nextest run -p llama-embedding -p model-loader -p ane-embedding`
      — the embedding stack still passes, proving the deletion did not reach it.
- [ ] Run `cargo nextest run -p acp-conformance` — the conformance suite still
      builds and passes without llama-agent.

## Workflow

- Use `/tdd` — there is no new behavior to pin here, so the proof is the three
  test runs above. Run them before and after so the before/after is on record.
#llama-agent #cleanup