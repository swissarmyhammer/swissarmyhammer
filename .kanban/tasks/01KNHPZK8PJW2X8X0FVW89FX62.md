---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8a80
title: 'Coverage: IndexContext::refresh stale-file branch (index.rs, ~8 lines)'
---
## What

`swissarmyhammer-treesitter/src/index.rs::IndexContext::refresh` has ~8 uncovered lines in the branch when a file's content hash has changed (stale file detection).

## Acceptance Criteria

- [ ] Test indexes a file via IndexContext
- [ ] Test modifies the file on disk (changing its content hash)
- [ ] Test calls `refresh()` and verifies the file is re-parsed with updated content
- [ ] The stale-file branch is exercised

## Tests

- [ ] Add test in `swissarmyhammer-treesitter/src/index.rs` (or `tests/`) that indexes a file, modifies it, calls `refresh()`, and asserts re-parsing occurred (e.g., new chunks reflect updated content)
- [ ] `cargo nextest run -p swissarmyhammer-treesitter` passes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #coverage-gap