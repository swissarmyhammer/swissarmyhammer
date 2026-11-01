---
assignees:
- claude-code
position_column: todo
position_ordinal: d080
title: 'swissarmyhammer-cli main.rs/cli_executor.rs: SERVE_COMMAND constant, nesting, ValueExtractor'
---
Eight findings across `apps/swissarmyhammer-cli/src/main.rs` and `cli_executor.rs`, split out of ^634hqth. All pre-existing — none sit in that commit's hunks.

## Items

- **`SERVE_COMMAND` constant** — the `"serve"` literal appears at two sites.
- **`display_validation_report` nesting** — too deep, extract.
- **`MAX_VALIDATION_WARNINGS_DISPLAY`** — magic number.
- **`handle_dynamic_matches` docs** — missing.
- **`ValueExtractor` consolidation** — duplicated extraction logic.
- **`ExecutionResult::error` signature** — flagged; judge what the engine wanted and record the reasoning.
- **`get_tool_registry_arc` rename** — overlaps ^yjk8kk0, which owns the `get_`-prefix sweep in `tool_registry.rs`. Do NOT rename it here; that card owns it and the call sites in this file are its blast radius.
- **`pub mod execute` doc** — missing.

## Overlap with ^pxr6rxe

`^pxr6rxe` already covers the magic numbers `10` and `5` and the `&Arc` parameters in this same file. Check it before starting; either merge the two cards or split the item list cleanly, but do not have two agents editing `main.rs` at once.

## Note on line numbers

Cited lines (280, 323, 374, 642, 710, and `cli_executor.rs` 33/254) track the pre-image and are offset. Grep for the symbol.

## Acceptance

- No repeated `"serve"` literal; no unnamed numeric literal at the cited sites.
- Max nesting 3 in `display_validation_report`.
- `cargo nextest run -p swissarmyhammer-cli`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean. #refactor #cli