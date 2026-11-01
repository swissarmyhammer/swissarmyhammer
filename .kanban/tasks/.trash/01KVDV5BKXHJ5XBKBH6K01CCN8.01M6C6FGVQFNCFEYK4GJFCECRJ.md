---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
title: Dedupe test-spec helpers in swissarmyhammer-lsp (fake_spec/test_spec) + name health-check interval constant
---
## What
Test-scaffolding cleanup surfaced by the `review working` engine while reviewing `^7a5h2bj` (leader-gated LSP spawn). These are **pre-existing test-code** items, not part of the leadership change, so they were deliberately split out rather than churning that task.

## Items
- [ ] `crates/swissarmyhammer-lsp/src/supervisor.rs` — `fake_spec` test helper is verbatim identical to `test_spec` in `daemon.rs` (test module). Two spec-builder fns in the same crate differing only by name → drift risk. Consolidate to one shared test helper (import `daemon::tests::test_spec` or hoist a shared `test_spec` into a common test-support location) and remove `fake_spec`.
- [ ] `crates/swissarmyhammer-lsp/src/supervisor.rs` — hardcoded `health_check_interval_secs: 1` literals in test specs (two sites). Mirror the existing `TEST_STARTUP_TIMEOUT_SECS` pattern: add `const TEST_HEALTH_CHECK_INTERVAL_SECS: u64 = 1;` and use it at both sites.

## Notes
- Pure test-code quality; no production behavior change. Keep tests fast (<10s). Run `cargo test -p swissarmyhammer-lsp` + clippy `-D warnings` green.
- Line numbers in the original findings were stale post-refactor; target by symbol. #diagnostics