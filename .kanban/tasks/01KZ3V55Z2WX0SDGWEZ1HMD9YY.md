---
assignees:
- claude-code
position_column: todo
position_ordinal: f480
title: Review fan-out forks spawn `claude` without `--model`; only the primed session carries the tier
---
## What

Measured from `.sah/*.log` on 2026-08-02: of 129 genuine
`🚀 Spawning Claude CLI argv:` records, only **2** carried `--model haiku`. The
other **127** were `--resume <uuid> --fork-session --session-id <uuid>` with no
`--model` switch at all.

The split is by attachment shape, not random:

| Spawn shape | Count | `--model haiku` |
|---|---|---|
| `--session-id <uuid>` (new session) | 2 | yes |
| `--resume <uuid> --fork-session` (fan-out) | 127 | no |

`ClaudeProcess::build_base_command`
(`crates/claude-agent/src/claude_process.rs:492`) appends `extra_args` for ALL
three `ConversationAttachment` shapes, including `Fork`. So a fork that spawns
without `--model` means its `SpawnConfig.extra_args` was EMPTY — the configured
tier does not reach the fork path. The fork path does not strip the flag; it
never receives it.

Decide which of these is true, then make the code state it plainly:

1. A forked session inherits the model from the parent's persisted session, so
   omitting `--model` is correct. Then document that at the fork arm in
   `build_base_command`, and add a test that pins it, so nobody "fixes" it later.
2. A forked session does NOT inherit the model. Then 127 of 129 review agents
   ran on the default Claude model instead of haiku, which is a cost and
   behavior defect. Thread `extra_args` down the fork path.

Determine the truth empirically — spawn a forked session and ask the model which
model it is, or inspect what the Claude CLI records for a resumed session. Do
not settle this by reading the ACP source alone.

Relevant code: `crates/claude-agent/src/claude_process.rs` (`build_base_command`,
`log_command`), the `SpawnConfig` that carries `extra_args`, and the review fleet
fan-out in `crates/swissarmyhammer-validators/src/review/`.

Context: this was found while auditing task ^hm82t0z, which replaced the
`claude-code-haiku` model-name lookup with a `ChatModelConfig { model }` field
whose `claude_args()` produces `["--model", "haiku"]`. That change is sound and
single-sourced; it does not cause this, and it does not fix it either. The same
fork seam is used before and after.

### Subtasks

- [ ] Determine empirically whether a forked Claude session inherits the parent's model.
- [ ] If it does not, thread `extra_args` to the fork spawn path.
- [ ] If it does, document it at the fork arm and pin it with a test.
- [ ] Add a test that asserts the fan-out spawn shape matches the decision.

## Acceptance Criteria

- [ ] There is a written, tested answer to whether a forked session inherits the
      model.
- [ ] Every `claude` process a review run spawns provably uses the resolved
      tier, either by carrying `--model` or by a documented and tested
      inheritance guarantee.
- [ ] A test fails if a future change makes the fan-out spawn on the default
      model when a tier is configured.

## Tests

- [ ] Add a test alongside the existing `build_base_command` tests in
      `crates/claude-agent/src/claude_process.rs` asserting the argv for
      `ConversationAttachment::Fork` matches the decided behavior.
- [ ] Add a review-level test that resolves the default review scope, drives the
      fan-out, and asserts every spawned argv carries the tier (or documents the
      inheritance path).
- [ ] Prove non-vacuous: make the fan-out drop the tier and confirm the new test
      fails.
- [ ] Run `cargo nextest run -p claude-agent -p swissarmyhammer-validators`.

## Workflow

- Use `/tdd` — establish the empirical answer first, then write the failing test.
#review #bug #config