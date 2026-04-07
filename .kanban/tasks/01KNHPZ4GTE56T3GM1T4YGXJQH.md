---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8880
title: 'Coverage: write_ts_symbols_and_edges (unified.rs, ~87 lines, 0%)'
---
## What

`swissarmyhammer-treesitter/src/unified.rs::write_ts_symbols_and_edges` has 0% coverage (~87 lines). This function writes tree-sitter symbols and call edges to the code-context database.

## Acceptance Criteria

- [ ] Integration test creates a CodeContextWorkspace (leader), indexes a source file with known symbols/call edges
- [ ] Test calls `write_ts_symbols_and_edges` and verifies symbols are written to the DB
- [ ] Test verifies call edges are written to the DB
- [ ] All existing tests continue to pass

## Tests

- [ ] Add integration test in `swissarmyhammer-treesitter/tests/` that creates a workspace, runs `build()`, and queries the DB for ts_symbols and call_edges tables
- [ ] `cargo nextest run -p swissarmyhammer-treesitter` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap