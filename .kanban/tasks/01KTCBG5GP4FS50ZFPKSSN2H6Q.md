---
assignees:
- claude-code
depends_on:
- 01KTCBDXJKQA68WPEE4MJW77ZH
position_column: todo
position_ordinal: '9780'
project: cli-schema-gen
title: Migrate sah dynamic_cli.rs to shared generator for per-op precise args
---
## What
Replace sah's imprecise global-union arg logic in `apps/swissarmyhammer-cli/src/dynamic_cli.rs` with the shared schema-driven generator (card B), so each verb advertises only its own operation's params with correct required flags.

The bug today: `create_command_data_from_tool` (`dynamic_cli.rs:1096`) reads `tool.operations()` (:1097) but computes args ONCE from the global `properties` block via `precompute_args(schema)` (:1103) and clones that full union onto every verb in `create_verb_command_data` (:1163, the clone at :1168). Result: every verb (e.g. `kanban task move`) advertises ALL kanban params and the required flags are wrong.

Approach: have sah build commands from each tool's FULL schema using `swissarmyhammer_operations::cli_gen::build_commands_from_schema`, and extract args with `::extract_noun_verb_arguments`, instead of the bespoke `create_noun_command_data`/`create_verb_command_data`/`precompute_args` union path. sah's command layer wraps clap in its own `CommandData`/`ArgData` structs — pick the lower-risk seam:
- Preferred: feed the per-op scoped args (from the shared generator's per-op `x-operation-schemas` read) into `CommandData`, replacing the global `schema_args` clone at :1168, so the rest of sah's dynamic-CLI plumbing is unchanged.
- Obtain the full schema in-process from `tool.operations()` via the `_full` generator (card C/D), NOT from the wire `schema()` (which is slim after card D).

Trace and preserve the dispatch side: wherever sah turns matched args back into the tool-call JSON, ensure it produces the same `{ "op": "verb noun", ...scoped args }` object the tools' forgiving `parse_input` expects.

Files: `apps/swissarmyhammer-cli/src/dynamic_cli.rs` (primary), plus any arg-extraction call site this feeds.

## Acceptance Criteria
- [ ] Each verb subcommand under a tool advertises ONLY its operation's params (per-op scoping), with that op's required fields enforced.
- [ ] `sah kanban task move` rejects invocations missing that op's required params and does NOT accept params belonging to other ops.
- [ ] The global-union path (`precompute_args` cloned onto every verb at :1168) is removed; full workspace builds.

## Tests
- [ ] Add/extend tests in `apps/swissarmyhammer-cli` (alongside existing `dynamic_cli` tests): assert that for a representative op (`move task`) the generated verb command's arg set equals that op's params (not the union), and that a required param is flagged required while a non-member param is absent.
- [ ] A negative test: parsing `kanban task move` without its required field errors; parsing with a foreign param errors (unknown arg).
- [ ] `cargo nextest run -p swissarmyhammer-cli dynamic_cli` passes.

## Workflow
- Use `/tdd` — write the per-op precision + required-enforcement tests first (they fail against today's union path), then swap in the shared generator.