---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: todo
position_ordinal: '9980'
project: cli-schema-gen
title: Add schema-driven shell op commands to shelltool-cli
---
## What
Surface the shell tool's operations as CLI commands in shelltool-cli, generated from the shell tool's FULL schema via the shared generator (card B). NOTE: unlike code-context-cli, shelltool-cli has NO operation commands today — `apps/shelltool-cli/src/cli.rs` only declares lifecycle commands (Serve/Init/Deinit/Doctor/Completion at :51-98) and there is no `commands/ops` module (`commands/mod.rs` only has doctor/registry/serve). So this card is ADDITIVE: it adds a `shell` op command tree (run/grep history/get lines/list processes/kill process — the `SHELL_OPERATIONS` in `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`).

Mirror kanban-cli's runtime split (because `cli.rs` is compiled standalone by `build.rs` via `#[path = "src/cli.rs"]` and may only depend on `clap` + `std`):
- Keep the static lifecycle `cli.rs` for build.rs doc/manpage/completion generation.
- In `apps/shelltool-cli/src/main.rs`, build the op subcommands at runtime from the shell tool full schema (`ShellTool::operations()` → `_full` generator → `swissarmyhammer_operations::cli_gen::build_commands_from_schema`), add them to the clap `Command`, and on a non-lifecycle match route through `extract_noun_verb_arguments` → `ShellTool::execute`.
- Add a `commands/ops.rs` (new) for the run_operation glue, registered in `commands/mod.rs`.

Changes:
- `apps/shelltool-cli/Cargo.toml`: add `swissarmyhammer-operations` dep (currently absent; depends on `swissarmyhammer-tools`/`swissarmyhammer-shell` only).
- `apps/shelltool-cli/src/main.rs`: switch from `Cli::parse()` (:42) to a built clap command that includes the schema-driven op subcommands, while preserving the lifecycle arms in `dispatch_command` (:123).
- `apps/shelltool-cli/src/commands/mod.rs` + new `ops.rs`.

If surfacing shell ops as a CLI is deemed out of scope (the brief flags H as adjustable), this card can be deferred — but the shared-generator infra (B) and the other migrations do not depend on it. Confirm with the user whether shelltool should gain a CLI op surface before implementing.

## Acceptance Criteria
- [ ] `shelltool` exposes its shell operations as schema-generated subcommands (e.g. `shelltool execute command --command "..."`, matching `SHELL_OPERATIONS` op strings).
- [ ] Lifecycle commands (serve/init/deinit/doctor/completion) still work; build.rs generation still compiles against the static `cli.rs`.
- [ ] A generated op invocation reaches `ShellTool::execute` and returns output.

## Tests
- [ ] Add `apps/shelltool-cli/src/commands/ops.rs` integration tests (mirroring code-context-cli's `ops.rs` tests): drive a shell op (e.g. `execute command`) through the schema-built tree + `extract_noun_verb_arguments` → `execute`, asserting output and exit code.
- [ ] Command-tree coverage test: generated nouns/verbs match `ShellTool::operations()` op strings.
- [ ] `cargo nextest run -p shelltool-cli` passes.

## Workflow
- Use `/tdd` — write the op-command coverage + execute round-trip tests first, then add the runtime schema-driven tree and ops glue.