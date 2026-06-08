---
assignees:
- claude-code
depends_on:
- 01KTMK4D4NV5FTM8V6YYCYHHGZ
position_column: todo
position_ordinal: '9880'
project: local-review
title: 'CLI: `sah model use <name> --for review`'
---
## What
Expose the per-purpose model selection from the config layer on the CLI so a user can run `sah model use <name> --for review` to set the review tool's model, while bare `sah model use <name>` keeps setting the global default.

Files:
- `apps/swissarmyhammer-cli/src/cli.rs`: add an optional `--for <purpose>` arg to the `Use` variant of `ModelSubcommand` (~line 426). Parse to a small enum/value-set; only `review` is valid for now (reject unknown purposes with a clear error). Absent `--for` = global default.
- `apps/swissarmyhammer-cli/src/commands/model/use_command.rs`: `execute_use_command` routes to `ModelManager::use_agent_for(name, target, &ModelPaths::sah())` based on `--for`. Keep the existing trim/empty-name handling and `ModelError` formatting.
- `apps/swissarmyhammer-cli/src/commands/model/show.rs`: extend `execute_show_command` to also display the review override (e.g. a "review: <name>" line, or "review: <default>" when unset, reading via `ModelManager::get_review_agent`).

## Acceptance Criteria
- [ ] `sah model use qwen-0.6b-test --for review` writes `review.model: qwen-0.6b-test` to `.sah/sah.yaml` and prints a success message naming the review scope.
- [ ] `sah model use claude-code` (no `--for`) still writes top-level `model:` unchanged.
- [ ] `--for <unknown>` is rejected with a clear, non-panicking error and a non-zero exit code.
- [ ] Empty/whitespace name still rejected as today.
- [ ] `sah model show` displays both the default model and the review override.

## Tests
- [ ] CLI test in `use_command.rs`: `--for review` writes `review.model` (assert file contents), default path unchanged.
- [ ] CLI test: unknown `--for` value yields an error result / non-zero exit, no panic.
- [ ] CLI test: empty name rejected.
- [ ] `show` test asserting both default and review lines render (unset review shows a default indicator).
- [ ] `cargo test -p swissarmyhammer-cli model` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.