---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffff8980
title: 'Coverage: build() full pipeline path (unified.rs, ~20 lines)'
---
## What

`swissarmyhammer-treesitter/src/unified.rs::build` has ~20 uncovered lines in the branch that calls `compute_unchanged_files` + `write_ts_symbols_and_edges`. This path is never reached in existing tests.

## Acceptance Criteria

- [ ] Integration test calls `workspace.build()` directly on a leader workspace with indexed files
- [ ] The compute_unchanged_files + write_ts_symbols_and_edges branch is exercised
- [ ] Coverage of the `build` function increases to cover those ~20 lines

## Tests

- [ ] Add integration test in `swissarmyhammer-treesitter/tests/` that opens a leader workspace, adds source files, calls `build()`, and verifies the full pipeline path executes (symbols/edges written)
- [ ] `cargo nextest run -p swissarmyhammer-treesitter` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap