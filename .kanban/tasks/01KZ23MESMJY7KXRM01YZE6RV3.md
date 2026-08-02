---
assignees:
- claude-code
position_column: todo
position_ordinal: ea80
project: drop-llama-agent
title: Remove the `sah model` CLI command
---
## What

Delete the `sah model` command. It exists to pick between chat models, and
Claude becomes the only chat executor.

- Delete `apps/swissarmyhammer-cli/src/commands/model/` — `mod.rs`,
  `list.rs`, `show.rs`, `use_command.rs`, `display.rs`, `description.md`
  (1873 lines).
- Remove the `model` entry from
  `apps/swissarmyhammer-cli/src/commands/registry.rs` and the `mod model;`
  declaration in `apps/swissarmyhammer-cli/src/commands/mod.rs`.
- Remove the `model` subcommand from the shell completions under
  `completions/` and from the man page `docs/sah.1`.
- Remove the `sah model` section from the book under `doc/src/`. Find it with
  `grep -rn "sah model" doc/ docs/ completions/`.

Keep the `--model` global flag handling in
`apps/swissarmyhammer-cli/src/main.rs` (`extract_model_flag`, line 290) for
now. The follow-up card that collapses the chat model configuration decides
its fate, and removing it here would break that card's callers.

### Subtasks

- [ ] Delete the `commands/model/` directory.
- [ ] Remove the registration and the module declaration.
- [ ] Remove the completions, man page, and book entries.
- [ ] Confirm no reference remains.

## Acceptance Criteria

- [ ] `apps/swissarmyhammer-cli/src/commands/model/` does not exist.
- [ ] `grep -rn "sah model" doc/ docs/ completions/` returns nothing.
- [ ] `sah --help` does not list a `model` subcommand.
- [ ] `cargo clippy -p swissarmyhammer-cli --all-targets -- -D warnings` exits
      0 with zero warnings.

## Tests

- [ ] Add a test in `apps/swissarmyhammer-cli/tests/` that runs the CLI with
      `--help` and asserts the output does NOT contain a `model` subcommand.
      Follow the pattern of the existing CLI tests in that directory.
- [ ] Add a test that running `sah model` exits non-zero with an
      unknown-subcommand error.
- [ ] Run `cargo nextest run -p swissarmyhammer-cli` — all tests pass and no
      test named `model_*` remains.

## Workflow

- Use `/tdd` — write the two CLI-surface tests first. They fail while the
  command still exists, and pass after it is deleted. #llama-agent #cli #cleanup