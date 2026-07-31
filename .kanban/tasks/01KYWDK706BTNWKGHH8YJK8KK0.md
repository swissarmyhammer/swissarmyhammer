---
assignees:
- claude-code
position_column: todo
position_ordinal: ce80
title: 'tool_registry.rs: drop get_ prefixes, derive Eq on ToolValidationSeverity'
---
Five findings in `crates/swissarmyhammer-tools/src/mcp/tool_registry.rs`, split out of ^634hqth. All pre-existing.

## Items

- Four `get_`-prefixed methods should drop the prefix, per Rust convention: cited at lines 721, 748, 768, 833 (pre-image numbers, verify by symbol).
- `ToolValidationSeverity` derives `PartialEq` without `Eq`. Cited at 1203 — that line sits in the block displaced by ^634hqth's +17-line trait-default insertion at 1046, so it is untouched pre-existing code.

## Blast radius — read before starting

The renames are **not** local. `get_tool_registry_arc` in particular is called from `apps/swissarmyhammer-cli/src/main.rs` (including the new `tool_property` helper) and `cli_executor.rs`. Run `code_context get blastradius` on each method before renaming, and expect to touch app crates.

Weigh whether the convention win is worth the churn on a method with many call sites — if a rename's blast radius is large and the name is unambiguous today, say so on the card and leave it, rather than doing a wide mechanical rename for style alone. Record the decision either way.

## Note on line numbers

Cited lines track the pre-image. Grep for the symbol.

## Acceptance

- No `get_` prefix on the four cited methods, or a recorded decision why a specific one stays.
- `ToolValidationSeverity` derives `Eq` alongside `PartialEq`.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Related: ^fjs8tqv also touches this file (prefix-match table, duplicate macros). Coordinate or sequence them — do not run both at once on the same file. #refactor