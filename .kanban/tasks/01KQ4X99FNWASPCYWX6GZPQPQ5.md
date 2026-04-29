---
assignees:
- claude-code
position_column: todo
position_ordinal: fa80
title: Wire up runtime `completion` subcommand for every CLI
---
## Problem

Each CLI binary should support `<bin> completion <shell>` to print a shell completion script to stdout. Today the wiring is inconsistent — completion scripts are generated at build time into `completions/`, but the runtime subcommand is missing or dead code.

## Per-crate status (verify before fixing)

- **swissarmyhammer-cli** (`sah`): `Commands::Completion { shell }` is defined in `swissarmyhammer-cli/src/cli.rs:243` and `print_completion()` exists in `swissarmyhammer-cli/src/completions.rs:25`, but:
  - `mod completions;` is **not** declared in `main.rs` or `lib.rs` — the module isn't compiled into the crate.
  - `route_subcommand` in `swissarmyhammer-cli/src/main.rs:542` has no arm for `"completion"`.
  - Result: `sah completion bash` is documented in `--help` but does nothing / errors.
- **kanban-cli**: has completion references in `cli.rs` and `main.rs` — verify it actually works end-to-end.
- **avp-cli, code-context-cli, shelltool-cli**: have `completion` references in `cli.rs` only — confirm whether the dispatch is wired in `main.rs`.
- **mirdan-cli**: no completion references in src/ at all — needs the subcommand added.

## Acceptance criteria

For each of the six CLIs (`sah`, `avp`, `code-context`, `kanban`, `mirdan`, `shelltool`):
1. `<bin> completion <shell>` prints a non-empty completion script to stdout for `bash`, `zsh`, `fish`, `powershell`.
2. The script registers under the **binary name** (e.g. `sah`, not `swissarmyhammer`) — match what's in `[[bin]] name` in `Cargo.toml`.
3. An integration test invokes the binary with each shell and asserts the output contains the binary name and is non-empty.
4. `--help` examples in the `Completion` subcommand's `long_about` use the correct binary name.

## Notes

- For `sah`, the immediate fix is: add `mod completions;` to `main.rs`, then add a `Some(("completion", sub_matches))` arm in `route_subcommand` that extracts the `shell` value and calls `completions::print_completion(shell)`.
- The build-time generation in each `build.rs` is already correct and uses the right binary names — leave it alone.
- The user already corrected `swissarmyhammer-cli/src/completions.rs` to use `"sah"` instead of `"swissarmyhammer"` as the program name (commit pending).