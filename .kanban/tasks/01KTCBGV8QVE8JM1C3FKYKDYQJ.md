---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: todo
position_ordinal: '9880'
project: cli-schema-gen
title: Migrate code-context-cli to schema-driven op commands
---
## What
Replace the hand-written operation subcommand enums in code-context-cli with schema-driven generation from the code_context tool's FULL schema (via card B's shared generator), eliminating the parallel hand-maintained command tree that can drift from the actual operations.

Today: `apps/code-context-cli/src/cli.rs` hand-declares `Commands` (the op groups `Get`/`Search`/`List`/`Grep`/`Query`/`Find`/`Rebuild`/`Clear`/`Lsp`/`Detect` at :87-137) and a sub-enum per group (`GetCommands` at :173, etc.), and `apps/code-context-cli/src/commands/ops.rs::run_operation` (:403) matches those variants to build the JSON args object then calls `CodeContextTool::execute` (:439).

Two constraints to respect:
1. `cli.rs` is compiled standalone by `build.rs` via `#[path = "src/cli.rs"]` for docs/manpages/completions and may only depend on `clap` + `std` (see the file header). So the schema-driven runtime tree must be built in `main.rs`/a runtime module, NOT inside `cli.rs`. Mirror kanban-cli's split: keep a static lifecycle `cli.rs` (Serve/Init/Deinit/Doctor/Skill/Completion) for build.rs, and build the op subcommands at runtime from the schema (kanban-cli does exactly this in `main.rs::build_cli`).
2. Preserve the dispatch path: `CodeContextTool::execute` is still the target. The hand-written per-variant arg mapping in `ops.rs::run_operation` is replaced by `swissarmyhammer_operations::cli_gen::extract_noun_verb_arguments` producing the `{ "op": ..., ...args }` object, which `execute` consumes.

Changes:
- `apps/code-context-cli/Cargo.toml`: add `swissarmyhammer-operations` dep (currently absent; it depends on `swissarmyhammer-tools` which re-exports operations — prefer a direct dep on `swissarmyhammer-operations` for `cli_gen`).
- `apps/code-context-cli/src/main.rs`: build the op command tree at runtime from `CodeContextTool` full schema (`tool.operations()` → `_full` generator → `build_commands_from_schema`); route non-lifecycle matches through `extract_noun_verb_arguments` → `execute`.
- Remove the op-group enums from `cli.rs` (`Get`/`Search`/.../`Detect` and their `*Commands` sub-enums) and the per-variant mapping in `commands/ops.rs`, keeping the lifecycle commands.

## Acceptance Criteria
- [ ] code-context op commands are generated from the schema; the hand-written `*Commands` op enums are gone from `cli.rs`.
- [ ] The generated command tree covers every code_context operation (get symbol/callgraph/blastradius/status, search symbol/code/workspace_symbol, list symbols, grep code, query ast, find duplicates, rebuild index, clear status, lsp status, detect projects, etc.).
- [ ] Lifecycle commands (serve/init/deinit/doctor/skill/completion) still work and build.rs doc/manpage/completion generation still compiles.
- [ ] Existing dispatch still works: a generated op invocation reaches `CodeContextTool::execute` and returns output.

## Tests
- [ ] Adapt the existing `run_operation` integration tests in `apps/code-context-cli/src/commands/ops.rs` (:918+, e.g. `test_run_operation_get_status`, `_json`) to drive through the schema-built command tree + `extract_noun_verb_arguments`, asserting the same end-to-end result (creation → execute → text/JSON → exit code).
- [ ] Add a command-tree coverage test asserting the generated nouns/verbs match `CodeContextTool::operations()` op strings.
- [ ] `cargo nextest run -p code-context-cli` passes.

## Workflow
- Use `/tdd` — write the command-tree coverage test + adapt the run_operation tests first, then build the runtime schema-driven tree and delete the hand-written enums.