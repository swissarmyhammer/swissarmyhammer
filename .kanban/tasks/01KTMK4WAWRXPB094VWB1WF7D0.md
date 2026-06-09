---
assignees:
- claude-code
depends_on:
- 01KTMK4D4NV5FTM8V6YYCYHHGZ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8c80
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
- [x] `sah model use qwen-0.6b-test --for review` writes `review.model: qwen-0.6b-test` to `.sah/sah.yaml` and prints a success message naming the review scope.
- [x] `sah model use claude-code` (no `--for`) still writes top-level `model:` unchanged.
- [x] `--for <unknown>` is rejected with a clear, non-panicking error and a non-zero exit code.
- [x] Empty/whitespace name still rejected as today.
- [x] `sah model show` displays both the default model and the review override.

## Tests
- [x] CLI test in `use_command.rs`: `--for review` writes `review.model` (assert file contents), default path unchanged.
- [x] CLI test: unknown `--for` value yields an error result / non-zero exit, no panic.
- [x] CLI test: empty name rejected.
- [x] `show` test asserting both default and review lines render (unset review shows a default indicator).
- [x] `cargo test -p swissarmyhammer-cli model` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes
- `--for` restricted to `review` at the clap layer in `dynamic_cli.rs` (`PossibleValuesParser`), so unknown purposes fail with exit 2. `target_for_purpose()` in `use_command.rs` guards the programmatic path too.
- `execute_use_command` gained a `for_purpose: Option<String>` parameter; routes via `ModelManager::use_agent_for`.
- `show.rs` `build_model_rows()` renders default + review rows; unset review shows `(uses default)`.
- Blast-radius consumers updated: `cli.rs` enum, `main.rs` parsing, `commands/model/mod.rs` routing, and `tests/integration/model_cli_parsings.rs` pattern match.

## Review Findings (2026-06-08 17:26)

### Nits
- [x] `apps/swissarmyhammer-cli/src/dynamic_cli.rs` (`MODEL_USE_LONG_ABOUT`, ~line 906) — The `model use` long help still only shows the two pre-existing examples (`sah model use claude-code`, `sah model use qwen`) and never mentions the headline feature this task added. Add an example such as `sah model use qwen --for review   # Set the review-tool model` so the documented behavior matches the new arg. RESOLVED: `MODEL_USE_LONG_ABOUT` now explains the `--for <purpose>` scoping and adds a `sah model use qwen --for review` example.
- [x] `apps/swissarmyhammer-cli/src/commands/model/use_command.rs` (`target_for_purpose`, ~line 60) and `apps/swissarmyhammer-cli/src/dynamic_cli.rs` (~line 2001) — The supported-purpose set (`"review"`) is hardcoded independently in two places: the `PossibleValuesParser` in the clap layer and the `match` in `target_for_purpose`. They will drift when a second purpose (e.g. `commit`) is added. Consider a single shared constant slice of supported purposes that both the parser and the matcher consume. RESOLVED: introduced `pub const SUPPORTED_PURPOSES: &[(&str, ModelTarget)]` plus `supported_purpose_names()` in `use_command.rs` as the single source of truth; `target_for_purpose` resolves via a lookup over it and the clap `PossibleValuesParser` consumes `supported_purpose_names()`. Adding a purpose is now a one-line edit. Covered by three new unit tests.