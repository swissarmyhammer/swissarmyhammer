---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6c237vv8bg3s1my9vq1exgp
  text: 'Trimmed dead parts after the removal of the files tool, shell tool and GUI app: tool_registry.rs is now the tool_registry/ directory; removed the ToolValidationSeverity Eq item and its acceptance line (Eq is already derived in validation.rs); removed the get_tool_registry_arc call-site claim (symbol no longer exists). The four cited get_ methods can no longer be identified; card lists the current candidates in registry.rs.'
  timestamp: 2026-11-01T17:21:42.779039+00:00
position_column: todo
position_ordinal: ce80
title: 'tool_registry: drop the get_ prefixes on the registry methods'
---
Findings in `crates/swissarmyhammer-tools/src/mcp/tool_registry/` (the old single file `tool_registry.rs` is now split into `registry.rs`, `tool.rs`, `validation.rs` and others), split out of ^634hqth. All pre-existing.

## Items

- Four `get_`-prefixed methods should drop the prefix, per Rust convention: cited at lines 721, 748, 768, 833 of the old single file (pre-image numbers, verify by symbol). The file is now split; `get_`-prefixed methods remain in `registry.rs` (`get_tool`, `get_tool_by_cli_name`, `get_cli_categories`, `get_tools_for_category`, `get_cli_tools`, `get_tool_validation_warnings`). Re-identify which four the finding meant.

## Blast radius — read before starting

The renames may not be local: callers live in `apps/swissarmyhammer-cli/src/` (for example `main.rs` and `cli_executor.rs`). Run `code_context get blastradius` on each method before renaming, and expect to touch app crates.

Weigh whether the convention win is worth the churn on a method with many call sites — if a rename's blast radius is large and the name is unambiguous today, say so on the card and leave it, rather than doing a wide mechanical rename for style alone. Record the decision either way.

## Note on line numbers

Cited lines track the pre-image. Grep for the symbol.

## Acceptance

- No `get_` prefix on the four cited methods, or a recorded decision why a specific one stays.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean.

Related: ^fjs8tqv also touches this file (prefix-match table, duplicate macros). Coordinate or sequence them — do not run both at once on the same file. #refactor