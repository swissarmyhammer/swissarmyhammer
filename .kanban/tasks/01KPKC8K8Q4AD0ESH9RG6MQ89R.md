---
assignees:
- claude-code
position_column: todo
position_ordinal: '9580'
title: 'clippy: 2 collapsible-if violations in swissarmyhammer-sem'
---
Pre-existing clippy errors surfaced by `cargo clippy --workspace --all-targets -- -D warnings`. Not regressions from the kanban review tasks.

Errors:
- swissarmyhammer-sem (lib): 2 occurrences of "this `if` can be collapsed into the outer `match`" (around line 163-171 in some file with `key_start = false;` inside a `match`)

Acceptance Criteria:
- Both `if` blocks collapsed into the surrounding `match` arms
- `cargo clippy -p swissarmyhammer-sem --all-targets -- -D warnings` passes

Tests: clippy is the test — re-run after change. Run `cargo clippy -p swissarmyhammer-sem --all-targets 2>&1` to see exact file:line. #test-failure