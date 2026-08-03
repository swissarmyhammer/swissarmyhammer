---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz2j3qs77ccy1s7zsa3gjxcr
  text: |-
    ### Closed by ^3y5n9g6 — the subject of this card no longer exists

    Card ^3y5n9g6 ("Delete the llama-agent crate") deleted every file this card
    names:

    - `apps/swissarmyhammer-cli/src/commands/agent/acp.rs` — gone, with
      `handle_command` and `build_agent_tools_mount`.
    - `apps/swissarmyhammer-cli/tests/integration/agent_command.rs` — gone.
    - The `Commands::Agent` short-circuit in
      `apps/swissarmyhammer-cli/tests/in_process_test_utils.rs` — gone, along with
      the `Commands::Agent` variant itself.

    The whole `sah agent acp` command went with the crate. `AgentToolsMount` and
    `InProcessMount` were defined only in `crates/llama-agent/src/mcp.rs`, and
    `AgentServer`, `AcpServer`, `AcpConfig`, and `PermissionPolicy` all came from
    llama-agent too, so there was nothing left to re-point the command at.

    Every acceptance criterion here is now unsatisfiable. Archiving.

    `apps/swissarmyhammer-cli/src/dynamic_cli_tests.rs::test_build_cli_has_no_agent_command`
    now guards the removal: it asserts the built command tree offers no `agent`
    subcommand. Verified RED against a re-injected `agent` subcommand, then GREEN.
  timestamp: 2026-08-03T00:59:19.207192+00:00
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