---
assignees:
- claude-code
depends_on:
- 01KW262DJSSC1JX2FXCDQN0X4E
- 01KW260M8QZ8T37A8RZGDDVZ81
- 01KW261K28N0RA00X9P0APED21
position_column: todo
position_ordinal: ad80
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