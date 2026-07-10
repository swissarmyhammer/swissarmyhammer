---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffec80
title: Clean up pre-existing workspace test failures (get cargo test --workspace fully green)
---
Standing list of pre-existing test failures observed during the plugin/cutover work (none introduced by it). Goal: `cargo test --workspace` fully green. Some are real bugs, some are test-isolation/env flakes — fix or correctly isolate each.

## Real bugs (fix the code or the test)
- **`swissarmyhammer-focus` `meta_snapshot::focus_tool_meta_operations_tree_is_complete`** — `generate sneak_codes` appears in the focus tool's `inputSchema` op enum but is MISSING from its `_meta` operations tree. Genuine schema drift: either add `sneak_codes` to the `_meta` tree (if it's a real op) or remove it from the enum. Make the `_meta` tree complete.
- **`swissarmyhammer-kanban` `derive_handlers::tests::apply_normalizes_slugs`** — slug-normalization logic/test mismatch. Investigate `derive_handlers` slug normalization; fix the logic or the expectation.

## Test-isolation / environment flakes (make deterministic; do NOT add prod APIs to fix test env — use CurrentDirGuard/serial_test/IsolatedTestEnvironment per the project's RAII rule)
- **`swissarmyhammer-kanban` `filter_integration::s17_tag_names_with_special_chars`** — fails deterministically in the full run / in isolation per reports; filter-DSL special-char handling. Confirm whether it's a real filter bug or a test-ordering issue, then fix.
- **`claude-agent` `session::tests::*` + `tools::tests::test_terminal_create_and_write` + `request_validation::tests::*`** (~24) — `Os NotFound` panics under the full `cargo test --workspace` run; PASS in isolation (`cargo test -p claude-agent --lib` = 696–720 passed). Cross-binary PTY/session-manager + HOME/CWD contention. Add proper isolation (serial_test / IsolatedTestEnvironment / unique temp HOME) so they pass under the full workspace run.
- **`swissarmyhammer-common` `test_isolated_test_environment_drop_restores_home`** and **mirdan-app lib** — HOME-isolation flakes under the parallel full run; pass in isolation. Same isolation treatment.

## Out of scope / accept
- **`llama-agent` `test_multi_turn_tool_use_round_trip_with_real_model`** — real-model nondeterminism; needs a model + is inherently flaky. Mark `#[ignore]` (real-model gate) rather than "fix", unless there's a deterministic harness.

## Note
The `example_layering_e2e::committed_examples_coload_across_layers` failure that was also on this list is ALREADY FIXED (01KT70DB) — don't re-add it.

## Acceptance
- `cargo test --workspace` green (real-model test may stay `#[ignore]` with justification).
- Isolation fixes use RAII/serial_test, not new production APIs.