---
assignees:
- claude-code
depends_on:
- 01KVTVACR1W8HFKFR8DTAJPMP0
position_column: todo
position_ordinal: a780
project: file-edit-tools
title: edit files — structured near-miss on no match (not "String not found")
---
## What
Replace the hard `"String '...' not found in file"` failure with a structured near-miss that shows the model exactly how its `find` diverged, so it corrects in one shot. In `crates/swissarmyhammer-tools/src/mcp/tools/files/edit/mod.rs`, consume `MatchOutcome::NoMatch { near }` from `swissarmyhammer-edit-match`.

- On no confident match, return the closest span(s): current text, line numbers, surrounding context, and a line-level diff between what the model supplied (`find`) and the nearest current text.
- In a multi-pair batch, return per-edit results so a single failed pair reports its own near-miss while the (atomic) batch as a whole does not commit.
- This is a structured tool result the model can act on, not an opaque `McpError` string.

## Acceptance Criteria
- [ ] A `find` with no confident match returns the nearest span(s) with current text, line numbers, context, and a diff vs the supplied `find`.
- [ ] In a multi-pair batch, the failing pair's near-miss is reported per-edit and the file is left byte-identical (atomic; coordinates with the cascade task).
- [ ] The legacy bare-"not found" error string is gone for this path.

## Tests
- [ ] Unit tests: near-miss payload contains current text + line numbers + diff; multi-edit per-edit failure reporting with byte-identical file.
- [ ] Update/replace the existing `test_edit_string_not_found` / `test_edit_empty_file` assertions to expect the structured near-miss.
- [ ] `cargo test -p swissarmyhammer-tools edit::` is green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.