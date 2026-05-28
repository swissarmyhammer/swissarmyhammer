---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
project: llama-coverage
title: Measure baseline coverage for llama-agent + ACP, produce the gap map
---
## What

Establish the coverage baseline for `crates/llama-agent` (including the `acp/` submodule) so the rest of this epic targets real gaps instead of guesses. The streaming 0-token bug proved that high test *volume* (819 lib tests) coexisted with zero coverage of the production streaming path — so we need a line/region map, not a test count.

## Steps

1. Pick the instrumentation already used in this repo. Check the `coverage` skill / existing CI config for `cargo-llvm-cov` vs `cargo-tarpaulin`. Use whatever is already wired; do not introduce a second tool.
2. Run coverage scoped to `llama-agent` only (the workspace run is huge and slow):
   - `cargo llvm-cov --package llama-agent --html` (or tarpaulin equivalent), plus a `--lcov`/`--json` export for diffing.
3. Produce a **per-file gap report** ranked by uncovered regions, with special attention to the large/critical files:
   - `generation/mod.rs`, `generation/generator.rs`
   - `stopper/*`
   - `queue.rs`
   - `chat_template.rs` (8.3k lines)
   - `acp/server.rs` (5.2k), `acp/translation.rs` (3k), `session.rs` (2.2k), `agent.rs` (2.4k)
4. For each major uncovered region, classify it: **pure logic** (testable now, no model), **model-dependent** (needs the scripted-model harness — card `<harness-id>`), or **dead/unreachable** (candidate for deletion).
5. Write the gap map into this task's comments/description as a checklist the downstream cards can consume. Do NOT create the downstream cards here — they already exist in this project; instead annotate which files each should target.

## Acceptance Criteria

- [ ] A coverage run for `llama-agent` completes and the tool + exact command are recorded in this task.
- [ ] A per-file uncovered-region report exists (committed as an artifact or pasted into the task), ranked worst-first.
- [ ] Each major gap is classified pure-logic / model-dependent / dead.
- [ ] Baseline overall % for the crate is recorded so the final coverage-gate card can set a threshold above it.

## Tests

- [ ] N/A — this is a measurement task, not a code change. The "test" is that `cargo llvm-cov --package llama-agent` runs to completion and emits a report.

## Workflow

- Use the `coverage` skill if it fits.
- This card unblocks the targeted-coverage cards; it does not itself add tests.