---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz1qperrhbjdabrx2gj16z4a
  text: |-
    Picked up. Research notes before the change:

    - The tool description reaches the agent through `include_str!("description.md")` in `McpTool::description` for `ShellExecuteTool`. One file, one surface.
    - `builtin/skills/shell/SKILL.md` has no Liquid partial include, so the rendered body equals the file body with the frontmatter stripped. The new `rendered_builtin_instructions("shell")` test therefore asserts against the file directly.
    - `.skills/` is generated and is not tracked by git. It was not touched.

    Deviation from the card, with the reason: the card asks the new tools test to call `ShellExecuteTool::new()`. `new()` calls `ShellState::new()`, which makes a `.shell` directory in the process CWD. Every other test in that `mod tests` block uses the test-only `ShellExecuteTool::new_isolated()`, which puts the state in a temp dir. The test uses `new_isolated()` to follow the prevailing pattern and to keep stray `.shell` dirs out of the repo. The acceptance criterion is worded as `ShellExecuteTool::description()`, which `new_isolated()` satisfies.

    TDD record (RED before GREEN):
    - `cargo nextest run -p swissarmyhammer-skills shell_output_guidance` failed with "builtin skill 'shell' must state that execute command blocks until the command exits".
    - `cargo nextest run -p swissarmyhammer-tools shell_description` failed with: shell tool description must contain "blocks until the command exits".

    The skills test function is named `shell_output_guidance_states_blocking_and_no_tail` so the card's filter command `cargo nextest run -p swissarmyhammer-skills shell_output_guidance` matches it. A nextest substring filter matches the test name, not the test binary name, so a function name without that substring makes the card's command run 0 tests.
  timestamp: 2026-08-02T17:17:41.016263+00:00
- actor: claude-code
  id: 01kz1rwwy6khm0k1rp36d7e2td
  text: |-
    Double-check found one true defect, and I corrected it.

    Finding: the first wording was factually wrong on the timeout path. Both `description.md` and `SKILL.md` said "The full output stays in the history" with no limit. In `execute_command/mod.rs`, `run` branches to `finalize_timed_out` on `RunOutcome::TimedOut`, and `finalize_timed_out` only calls `mark_timed_out`. `store_command_output` is called only by `finalize_completed`. `tokio::time::timeout` drops the `execute_with_guard` future, so the buffered lines never reach `ShellState`. A timed-out command stores zero lines, and the response body has no output block. An agent that read the new text and then hit a timeout would call `get lines` for output that does not exist — the same trap the card is trying to close, inverted.

    Fix, applied to the whole of each file, not to the one sentence in the finding:
    - `description.md` — lead sentence now reads "Every command that exits stores its full output"; paragraph 2 splits the exit case from the timeout case and states "no output is stored".
    - `SKILL.md` — lead sentence, the `### execute command` line, the `## How output works` section, and the `## Timeout` section all state the limit.

    TDD for the correction as well: added the `no output is stored` assertion to both tests and watched both fail first.
    - skills: "builtin skill 'shell' must state that a timed-out command keeps nothing ('no output is stored') ..."
    - tools: shell tool description must contain "no output is stored"

    Other double-check results, all clean: no stale copy of the old text anywhere in the repo, no snapshot or golden of the shell description, `.skills/` is gitignored and absent on disk, and the GFM table with `\|` inside code spans renders correctly.

    Discovered work, filed as a separate card ^0dystg9: `cargo nextest run -p swissarmyhammer-tools` creates a `.shell` directory in `crates/swissarmyhammer-tools/` because `tool_config.rs` tests and `tests/integration/file_size_limits.rs` call `ShellExecuteTool::new()` instead of `new_isolated()`. Pre-existing, not caused by this card.
  timestamp: 2026-08-02T17:38:40.710297+00:00
- actor: claude-code
  id: 01kz1rx4ge10s1dfnqhakgwqbp
  text: |-
    ### implement — changed
    - evidence: 5 files — crates/swissarmyhammer-skills/tests/shell_output_guidance.rs (new), crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md, crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs, builtin/skills/shell/SKILL.md. `cargo nextest run --workspace` — 15258 tests run: 15258 passed, 2 skipped. `cargo clippy --workspace --all-targets -- -D warnings` — clean. `cargo fmt --all` — clean.
    - next: ready for /review
  timestamp: 2026-08-02T17:38:48.462228+00:00
position_column: doing
position_ordinal: '8380'
title: 'shell tool: say that `execute command` blocks to completion, and stop `| tail` pipelines'
---
## What

The `shell` MCP tool runs every command to completion. `execute_command::run`
(`crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:79`)
returns only after the child process exits or the timeout kills it. The full
output goes to the shell log, and the response shows the last 32 lines.

The agent-facing text does not say this. The tool description
(`crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md`) is one
sentence. The skill (`builtin/skills/shell/SKILL.md:18`) has only a weak
bullet: "skip `| tail` / `| grep` pipelines". Agents thus keep writing
`cmd 2>&1 | tail -60`, which throws away output that the tool already keeps.

Make three text surfaces state the same two facts:

1. `execute command` blocks until the command exits or the timeout kills it.
2. Do not pipe to `tail`, `head`, or `grep`. Read the output later with
   `get lines` or `grep history`.

Use these two marker sentences verbatim in each surface, because the tests
below assert on them:

- `blocks until the command exits`
- ``Do not pipe to `tail` ``

Files to change:

- `crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md` — add the
  two facts. This file is the tool description through `include_str!` at
  `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:500`.
- `crates/swissarmyhammer-tools/src/mcp/tools/shell/execute_command/mod.rs:56`
  — change the `Operation::description()` string from
  "Execute a shell command with timeout and environment control" to also say
  the op blocks until the command exits.
- `builtin/skills/shell/SKILL.md` — replace the weak bullet at line 18 with a
  clear section that gives both facts. Do NOT edit `.skills/`; that directory
  is generated from `builtin/skills/`.

### Subtasks

- [x] Write the two failing tests first (see **Tests**).
- [x] Update `description.md` with the blocking fact and the no-pipe rule.
- [x] Update the `ExecuteCommand::description()` string.
- [x] Update `builtin/skills/shell/SKILL.md`.

### Added during implementation — the timeout limit

A double-check found that the first wording was false on the timeout path.
`finalize_timed_out` only calls `mark_timed_out`; `store_command_output` runs
solely in `finalize_completed`, and `tokio::time::timeout` drops the buffered
output. A command the timeout kills therefore stores nothing, and `get lines`
and `grep history` find nothing for it. Text that promises stored output
without that limit sends the agent to read output that was never written.

- [x] All three surfaces state the limit with the marker `no output is stored`.
- [x] Both tests assert the marker.
- [x] The same unqualified promise removed from the whole of each file — the
      lead sentence of `description.md`, the lead sentence and the
      `### execute command` line of `SKILL.md`, and the `## Timeout` section.

## Acceptance Criteria

- [x] `ShellExecuteTool::description()` contains `blocks until the command exits`.
- [x] `ShellExecuteTool::description()` contains ``Do not pipe to `tail` ``.
- [x] `ShellExecuteTool::description()` names both `get lines` and
      `grep history` as the way to read output after the command ends.
- [x] `ExecuteCommand::description()` says the op blocks until the command exits.
- [x] The rendered `shell` skill body contains both marker sentences.
- [x] `builtin/skills/shell/SKILL.md` no longer contains the old weak bullet
      text "skip `| tail` / `| grep` pipelines".
- [x] Both new tests pass; no other test in the workspace breaks.

## Tests

- [x] New file `crates/swissarmyhammer-skills/tests/shell_output_guidance.rs`,
      modeled on `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs`.
      Use `mod common;` and `use common::rendered_builtin_instructions;`. Assert
      that `rendered_builtin_instructions("shell")` contains
      `blocks until the command exits` and ``Do not pipe to `tail` ``, and that
      it does not contain `skip `| tail``.
- [x] New test `shell_description_states_blocking_and_no_tail` in the `mod tests`
      block of `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`. Assert
      that `ShellExecuteTool::new().description()` contains both marker
      sentences plus `get lines` and `grep history`, and that
      `super::EXECUTE_CMD.description()` contains `blocks until the command exits`.
- [x] Run `cargo nextest run -p swissarmyhammer-skills shell_output_guidance` —
      expect the new test to fail before the text changes and pass after.
- [x] Run `cargo nextest run -p swissarmyhammer-tools shell_description` —
      expect the new test to fail before the text changes and pass after.

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
#docs #shelltool #tools