---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: remove-prompts
title: Remove the `sah prompt` CLI subcommand
---
## What
Remove the entire `sah prompt` CLI subcommand (list / test / show / new / edit) and its wiring. This is a leaf removal — nothing else in the codebase depends on these command modules.

Files to delete:
- `apps/swissarmyhammer-cli/src/commands/prompt/` (whole dir: `mod.rs`, `cli.rs`, `list.rs`, `show.rs`, `test.rs`, `new.rs`, `edit.rs`, `display.rs`, `description.md`, `list_help.md`, `test_help.md`) — ~2867 LOC

Files to edit:
- `apps/swissarmyhammer-cli/src/main.rs` — remove `handle_prompt_command` (around line 1279) and its dispatch arm `Some(("prompt", sub_matches)) => ...` (around line 548).
- `apps/swissarmyhammer-cli/src/cli.rs` — remove the `prompt` subcommand definition and update the top-level about/help text (lines ~70-213) that describe sah as "An MCP server for managing prompts as markdown files" and the `prompt list`/`prompt test` examples. Reframe help around skills/workflows.
- `apps/swissarmyhammer-cli/src/commands/mod.rs` — remove the `pub mod prompt;` declaration.
- `apps/swissarmyhammer-cli/src/list.rs` and `apps/swissarmyhammer-cli/src/mcp_integration.rs` — remove prompt-specific listing paths that feed the prompt command (keep skill/workflow listing).

Keep: the CliContext prompt_library field is still needed for skill rendering at this stage — do NOT remove it here; that is handled in the later library-rename task. Only remove the user-facing `prompt` command surface.

## Acceptance Criteria
- [ ] `apps/swissarmyhammer-cli/src/commands/prompt/` directory no longer exists.
- [ ] `sah --help` output contains no `prompt` subcommand.
- [ ] `sah prompt` returns an unknown-subcommand error.
- [ ] `cargo build -p swissarmyhammer-cli` succeeds.
- [ ] Top-level CLI about/help text no longer markets "managing prompts".

## Tests
- [ ] Update/remove `apps/swissarmyhammer-cli/tests/integration/prompt_performance.rs` (delete — it tests the prompt command).
- [ ] Add an integration assertion in the CLI test suite that running the binary with `prompt` exits non-zero and stderr mentions an unrecognized subcommand (e.g. in `apps/swissarmyhammer-cli/tests/integration/`).
- [ ] `cargo test -p swissarmyhammer-cli` is green.

## Workflow
- Use `/tdd` — write the failing "no prompt subcommand" assertion first, then delete code to make it pass.