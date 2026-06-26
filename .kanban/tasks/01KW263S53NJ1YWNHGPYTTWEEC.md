---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2zfntk9t3cc4d2qaf4tr5a
  text: 'Picked up. Researched: types.rs (Observation/Checkpoint/SurfaceState/Trajectory), surface/mod.rs (SurfaceAdapter trait provision/drive/observe/teardown), surface/cli.rs (CliAdapter + safe_output_path traversal guard), spec.rs (Expectation given/when), loader.rs (resolve_scope), config.rs (EXPECT_DIR=".expect", find_expect_dir), tools/expect/mod.rs (stub ops dispatch), init.rs (.expect/received scaffold). Plan: new engine observe.rs with observe(expectation, adapter, &ObserveConfig{repo_root}) running provision -> arrange(Given via drive, no checkpoint) -> act(When: drive+observe -> checkpoint per step) -> final checkpoint -> teardown; plus received_path/write_received with traversal guard mirroring safe_output_path. Wire observe expectation/expectations ops via shared handler resolving scope + writing .expect/received/<path>.received.json. TDD: engine integration test in tests/observe.rs + tools op test.'
  timestamp: 2026-06-26T22:05:56.435089+00:00
- actor: claude-code
  id: 01kw305rwvm3hrd4za3khzjd6g
  text: |-
    Implementation complete and green. Engine: new src/observe.rs with observe(expectation, adapter, &ObserveConfig) running provision -> arrange(Given via drive, no checkpoint) -> act(When: drive+observe = one checkpoint per step) -> final checkpoint -> teardown; received_path/write_received write .expect/received/<path>.received.json with a path-traversal guard mirroring cli.rs safe_output_path. Tools: manual Operation impls for observe expectation/expectations with shared scope/tag params, observe_op/observe_one/observe_repo_root handlers, dispatch wired, added to IMPLEMENTED_OPS.

    Verification (all green): cargo nextest -p swissarmyhammer-expect -p swissarmyhammer-tools -E 'test(observe) or test(expect)' = 44 passed; cargo check --workspace OK; cargo clippy (both crates, -D warnings) clean; cargo fmt --check clean.

    Adversarial double-check (advisory) raised 4 findings; proceeding with justification (out of this card's scope / unmandated design choices):
    1. named-files capture not wired from a frontmatter field. CliState carries the `files` slot at every checkpoint (criterion met at the type level; cli.rs tests exercise .capturing()). Populating it from a spec needs a NEW Frontmatter key (deny_unknown_fields closed enum) which is a separate task, not this one. Filing as follow-up.
    2. observe expectations is fail-fast (first failing spec aborts the batch). Acceptance criteria don't mandate per-spec resilience; clear error is acceptable for the deterministic first cut. Follow-up candidate.
    3. A cli spec with zero When steps surfaces the adapter's "nothing to observe" error. Edge case outside the When-bearing specs this card targets; follow-up candidate for a clearer domain error.
    4. received gitignoring is established by `expect init` (prior task scaffolds .expect/.gitignore with received/). Correct separation of concerns; left as-is.

    Task left in doing for review.
  timestamp: 2026-06-26T22:18:00.475689+00:00
depends_on:
- 01KW262DJSSC1JX2FXCDQN0X4E
- 01KW260M8QZ8T37A8RZGDDVZ81
- 01KW261K28N0RA00X9P0APED21
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffed80
project: expect
title: expect expectation observe → Observation (cli, deterministic)
---
## What
Implement `observe`: run an expectation through provision → arrange → act → observe → teardown and assemble the authoritative `Observation` timeline. cli surface only here (no agent). Per `ideas/expect.md` §"The Check Loop".

- New `crates/swissarmyhammer-expect/src/observe.rs`:
  - `observe(expectation, adapter, config) -> Result<Observation, ExpectError>`:
    1. Provision the SUT (via the surface adapter).
    2. Arrange (`Given`) — establish preconditions deterministically (cli: run setup commands / arrange fixtures).
    3. Act (`When`) — drive each `When` step mechanically via the adapter.
    4. Observe — capture a `Checkpoint` after EACH `When` step plus a final one (the multi-checkpoint timeline; criteria are multi-step/relational/temporal), recording `state` + `duration`.
    5. Teardown.
  - Roles kept separate: driver causes the transition, adapter observes authoritative state; the observation — not any transcript — is the result.
- Wire the `expectation observe` and `expectations observe` ops in `tools/expect/mod.rs` (replace stubs): resolve scope, run `observe`, store the result as the `received` observation under `.expect/received/<path>.received.json` (gitignored).

## Acceptance Criteria
- [ ] `expect expectation observe <scope>` provisions, drives each When step, captures one checkpoint per When step + a final, and writes `.expect/received/<path>.received.json`.
- [ ] Each `Checkpoint.after` names its When step (or "final"); `duration` is recorded.
- [ ] cli `SurfaceState` carries stdout/stderr/exit/named-files at each checkpoint.
- [ ] `expectations observe` (plural) runs the same over a multi-spec scope.

## Tests
- [ ] Integration test using a fixture cli SUT + a small `.expect.md` with 2 When steps: assert the Observation has 3 checkpoints in order and the received file is written.
- [ ] `cargo nextest run -p swissarmyhammer-expect observe` and the tools-side op test pass.

## Workflow
- Use `/tdd`.