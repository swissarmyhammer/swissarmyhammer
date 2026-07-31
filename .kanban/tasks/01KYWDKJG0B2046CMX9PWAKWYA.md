---
assignees:
- claude-code
position_column: todo
position_ordinal: cf80
title: 'ralph/execute: dispatch from RALPH_OPERATIONS, define_operation! macro, DEFAULT_MAX_ITERATIONS'
---
Five findings in `crates/swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs`, split out of ^634hqth. All pre-existing.

## Items

1. **Dispatch is hardcoded on op strings** rather than driven from `RALPH_OPERATIONS`. Highest-value item — this is the same class of drift that ^6xjxebg had to fix twice in the CLI registry: a declared roster and a separate hardcoded list that can disagree with no error.
2. **`define_operation!` macro** — the per-operation boilerplate repeats; a macro or table removes it.
3. **`DEFAULT_MAX_ITERATIONS`** — the `50` literal should be named.
4. **Session-id extraction helper** — repeated across the op arms. Note ^j0rkmeg needs a single validation point for `session_id`; that extraction helper is the natural home, so sequence this card AFTER ^j0rkmeg or coordinate, and do not build two competing helpers.
5. **`Debug` derive** missing.

## Acceptance

- Adding an op to `RALPH_OPERATIONS` cannot leave dispatch stale — a test proves the dispatch set and the roster agree.
- No bare `50` literal.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Dropped from this list, deliberately: a finding asking to extract a constant for `max_iterations = 25` in an existing test. The review skill exempts restyling pre-existing test code. #refactor