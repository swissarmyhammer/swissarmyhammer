---
assignees:
- claude-code
position_column: todo
position_ordinal: c980
title: 'swissarmyhammer-cli main.rs: magic numbers 10 and 5, &amp;Arc parameters'
---
Three style items in `apps/swissarmyhammer-cli/src/main.rs`. Split out of ^6xjxebg — all pre-existing.

## Items

1. **Magic number `10`** — unnamed numeric literal, give it a named constant.
2. **Magic number `5`** — `report_validation_issues(&cli_builder, false, 5)`. Name it.
3. **`&Arc<...>` parameter** — take `&T` or clone at the call site.

## Note on attribution

The `5` finding is a good illustration of why this card exists. The engine cited `main.rs:351`, which resolves to a line inside ^6xjxebg's hunk *range* — but it is a context line. That commit inserted `relax_required_tool_args` above it, which shifted the line down; the call itself is untouched pre-existing code. A validator reading the diff hunk saw a "new" line number and reported it as new work.

Expect the same when working this card: the engine's cited line numbers track the pre-image and are frequently offset. Grep for the symbol.

## Acceptance

- No unnamed numeric literal in the cited call sites; each has a constant whose name says what the number means.
- `cargo nextest run -p swissarmyhammer-cli`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.