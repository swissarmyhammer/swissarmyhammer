---
assignees:
- claude-code
position_column: todo
position_ordinal: ef80
title: '`sah agent acp` has parse-only test coverage; the harness short-circuits the production path'
---
## What

No test in the workspace invokes `handle_command` or `build_agent_tools_mount`
in `apps/swissarmyhammer-cli/src/commands/agent/acp.rs`. Coverage is parse-only.

`apps/swissarmyhammer-cli/tests/integration/agent_command.rs::test_agent_acp_command_parsing`
looks like it drives the real path. It does not. The harness
`execute_cli_command_with_capture` in
`apps/swissarmyhammer-cli/tests/in_process_test_utils.rs` (lines 377-391)
special-cases `Commands::Agent { subcommand: Some(_) }` and returns
`EXIT_SUCCESS` at once. It never calls `handle_command`. The comment there says
"just confirm parsing succeeded".

This was found while `build_agent_tools_mount` moved from
`crates/swissarmyhammer-agent/src/lib.rs` into the CLI (task ^6s0py85). The move
was proved correct by a line-by-line diff, because no test could prove it.

Give the ACP agent path a real test. Remove the short-circuit in the harness, or
add a test that calls `handle_command` directly and asserts the mount is built
and the tools server starts.

### Subtasks

- [ ] Add a test that calls the real `handle_command` for `sah agent acp`.
- [ ] Assert `build_agent_tools_mount` returns a usable mount.
- [ ] Remove or narrow the `Commands::Agent` short-circuit in the harness.
- [ ] Prove the new test is not vacuous.

## Acceptance Criteria

- [ ] A test calls `handle_command` in
      `apps/swissarmyhammer-cli/src/commands/agent/acp.rs` and asserts on its
      result, not only on argument parsing.
- [ ] `execute_cli_command_with_capture` no longer returns `EXIT_SUCCESS` for
      every `Commands::Agent` subcommand, or the new test bypasses that harness.
- [ ] Breaking `build_agent_tools_mount` on purpose makes the new test fail.

## Tests

- [ ] Add the test in
      `apps/swissarmyhammer-cli/tests/integration/agent_command.rs`.
- [ ] Prove it is not vacuous: make `build_agent_tools_mount` return an error,
      watch the new test fail, then revert. Record both results.
- [ ] Run `cargo nextest run -p swissarmyhammer-cli` — all tests pass.

## Workflow

- Use `/tdd` — write the failing test first.
#coverage-gap #cli #test