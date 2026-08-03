---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz50xr3h64ctw6sakgnwntny
  text: |-
    ### implement — changed

    Verified empirically (not just by reading the diff) that `^j9rwjtx`'s fix (Session.extra_args captured at session/new, read back by build_fork_spawn_config) actually closes this task's measured symptom, by driving the REAL production chain end to end in a new test: `create_new_session_internal` (session/new) → prime → `fork_session` (session/fork ext) → `build_fork_spawn_config` → `ClaudeProcess::build_base_command`, then reading the resulting argv.

    Two real, specific test gaps existed and are now closed:
    - `crates/claude-agent/src/claude_process.rs`: added `test_base_command_fork_attachment_carries_extra_args` — the exact test the card's "Tests" checklist named (argv for `ConversationAttachment::Fork` carrying `--model`), which did not exist before (the existing `extra_args` argv test used `New`; the existing `Fork` argv test used empty `extra_args`).
    - `crates/claude-agent/src/session_fork.rs`: added `test_review_fanout_chain_carries_model_tier_to_forked_argv` — drives the real `session/new` → `session/fork` chain (not a hand-set `Session.extra_args` like the pre-existing tests) all the way to the assembled argv. This is the closest achievable stand-in for a "review-level fan-out" test: `swissarmyhammer-validators`'s own fleet tests use a fully scripted `ScriptedAgent` double that never models `claude` argv, so no test in that crate can assert on `--model` — documented on the task description and in the test's doc comment.

    Made `ClaudeProcess::build_base_command` `pub(crate)` (was private) so the session_fork.rs test could reach it.

    Non-vacuous proof: temporarily reverted `build_fork_spawn_config`'s `.extra_args(parent.extra_args.clone())` to `.extra_args(Vec::new())` — 3 tests failed (the 2 new plus the existing `test_fork_spawn_config_carries_extra_args`/`..._not_live_config`), then restored. Separately, temporarily made `build_base_command` skip appending `extra_args` on the `Fork` arm — the 2 argv-level tests failed with the expected panic message, then restored. `git status`/`git diff` confirm both source files are back to the intended final state (no stray `.bak` files).

    Evidence:
    - `cargo nextest run -p claude-agent -p swissarmyhammer-validators` → 1446 tests run, 1446 passed, 0 skipped.
    - `cargo nextest run -E 'rdeps(claude-agent) or rdeps(swissarmyhammer-validators)'` → 4400 tests run, 4400 passed, 0 skipped.
    - `cargo fmt --all -- --check` clean after `cargo fmt --all`.
    - `cargo clippy -p claude-agent -p swissarmyhammer-validators --all-targets -- -D warnings` clean.

    Files changed: `crates/claude-agent/src/claude_process.rs`, `crates/claude-agent/src/session_fork.rs`.

    next: /review
  timestamp: 2026-08-03T23:56:40.433922+00:00
position_column: doing
position_ordinal: '8280'
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

## Resolution (verified empirically, `/implement`)

Answer 2 was already true, and already fixed by task `^j9rwjtx` (commits
`0c3947a9d`, `d2927bfb0`, `ae4f478d5`): `Session.extra_args` is captured from
the agent's live config at `session/new` time
(`ClaudeAgent::store_extra_args_in_session`), and
`ClaudeAgent::build_fork_spawn_config` (`crates/claude-agent/src/session_fork.rs`)
now reads `parent.extra_args.clone()` instead of leaving it empty. This was
verified, not assumed, by driving the REAL production chain (`session/new` →
`fork_session` → `build_fork_spawn_config` → `ClaudeProcess::build_base_command`)
in a new test and reading the resulting argv.

Two real test gaps remained and are now closed:

1. No test asserted the literal argv for `ConversationAttachment::Fork` with a
   non-empty `extra_args` — every existing `extra_args` argv test used `New`;
   the existing `Fork` argv test used empty `extra_args`. Closed by
   `test_base_command_fork_attachment_carries_extra_args` in
   `crates/claude-agent/src/claude_process.rs`.
2. No test drove the real `session/new` → `session/fork` chain end-to-end to
   the argv; `session_fork.rs`'s existing tests set `Session.extra_args`
   directly via `update_session`, bypassing the real capture path, and never
   checked the assembled argv. Closed by
   `test_review_fanout_chain_carries_model_tier_to_forked_argv` in
   `crates/claude-agent/src/session_fork.rs`.

`swissarmyhammer-validators`' own fan-out tests (`review/fleet/tests.rs`) drive
a fully scripted `ScriptedAgent` test double, not a real `ClaudeAgent` — that
double does not model `claude` argv at all, so no test at that layer can
assert on `--model`. The real argv only exists inside `claude-agent`; the
validators crate talks to it only through the backend-agnostic ACP
`session/fork` extension. The new `session_fork.rs` test above is therefore
the most faithful "drives the actual fan-out" proof achievable without a real
`claude` binary on `PATH` (the one line it cannot drive through production
code is setting `SpawnConfig.attachment = Fork`, which production sets in
`ClaudeClient::fork_process` immediately before spawning a real process).

Both new tests were proven non-vacuous: reverting `build_fork_spawn_config`'s
`extra_args` to empty, and separately reverting `build_base_command` to skip
`extra_args` on `Fork`, made the new tests (and existing ones) fail with the
expected message; both reverts were then undone.

### Subtasks

- [x] Determine empirically whether a forked Claude session inherits the parent's model. (It does not inherit automatically — the CLI needs `--model` passed explicitly; the FIX is that the fork spawn now carries the parent's captured `extra_args`, which includes `--model`.)
- [x] If it does not, thread `extra_args` to the fork spawn path. (Already done by `^j9rwjtx`; verified still correct.)
- [x] If it does, document it at the fork arm and pin it with a test. (N/A — it does not inherit; see above.)
- [x] Add a test that asserts the fan-out spawn shape matches the decision.

## Acceptance Criteria

- [x] There is a written, tested answer to whether a forked session inherits the
      model.
- [x] Every `claude` process a review run spawns provably uses the resolved
      tier, either by carrying `--model` or by a documented and tested
      inheritance guarantee.
- [x] A test fails if a future change makes the fan-out spawn on the default
      model when a tier is configured.

## Tests

- [x] Add a test alongside the existing `build_base_command` tests in
      `crates/claude-agent/src/claude_process.rs` asserting the argv for
      `ConversationAttachment::Fork` matches the decided behavior.
- [x] Add a review-level test that resolves the default review scope, drives the
      fan-out, and asserts every spawned argv carries the tier (or documents the
      inheritance path). (Documented as a `claude-agent`-level end-to-end test —
      see Resolution note on why `swissarmyhammer-validators`'s own fan-out tests
      cannot assert on argv.)
- [x] Prove non-vacuous: make the fan-out drop the tier and confirm the new test
      fails.
- [x] Run `cargo nextest run -p claude-agent -p swissarmyhammer-validators`.

## Workflow

- Use `/tdd` — establish the empirical answer first, then write the failing test. #bug #config #review