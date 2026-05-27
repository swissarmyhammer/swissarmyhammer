---
assignees:
- claude-code
position_column: todo
position_ordinal: '8480'
title: 'Flaky: avp-common validator::runner::tests::test_execute_ruleset_runs_rules_in_parallel'
---
## What

`avp-common::validator::runner::tests::test_execute_ruleset_runs_rules_in_parallel` failed under the full `cargo nextest run --workspace` run (14663 tests, ~452s wall time, high parallel load) but passes deterministically when run in isolation (`cargo nextest run -p avp-common <name>` → ok in 0.4s).

Classic flake: timing-sensitive parallel-execution assertion that relies on wall-clock to verify rules run concurrently. Under heavy CI/load the parallelism window narrows and the assertion misses.

## Where

- File: `crates/avp-common/src/validator/runner.rs`
- Test: `validator::runner::tests::test_execute_ruleset_runs_rules_in_parallel`

## Acceptance Criteria

- Reproduce the failure under load (e.g. `for i in {1..50}; do cargo nextest run -p avp-common <name>; done` while running unrelated builds in parallel).
- Remove the timing dependency: instead of asserting wall-clock concurrency, instrument the runner to record start/finish events per rule and assert overlap structurally (e.g. all rules' start events precede any rule's finish event, via a shared barrier or sequence log).
- Test must pass 100 runs in a row, both alone and inside `cargo nextest run --workspace`.

## Tests

- `cargo nextest run -p avp-common validator::runner::tests::test_execute_ruleset_runs_rules_in_parallel` — single test
- `cargo nextest run --workspace` — full suite green #test-failure