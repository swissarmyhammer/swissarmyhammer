---
assignees:
- claude-code
position_column: todo
position_ordinal: d580
title: Reduce cognitive complexity in EntityContext (context.rs)
---
## What

Five functions in `swissarmyhammer-entity/src/context.rs` exceed cognitive complexity thresholds flagged by the `code-quality:cognitive-complexity` validator:

| Function | Nesting | Branches | Issue |
|----------|---------|----------|-------|
| `process_attachment_field` | 8 | 22 | Deeply nested match/if with 8 levels |
| `write_internal` | 5 | 20 | Triple-nested if-let blocks for changelog handling |
| `derive_compute_fields` | 4 | 17 | Repeated cache warmth logic |
| `validate_for_write` | 4 | 17 | Multiple loops with nested conditionals |
| `enrich_attachment_fields_with_defs` | 7 | 15 | for loop with nested if-let reaching 7 levels |

The threshold is 3-level nesting. All five functions exceed it.

## Approach

Refactor each function to reduce nesting and branching without changing behavior:

- Extract helper functions for deeply nested branches
- Use early returns / `continue` to flatten conditionals
- Consider extracting match arms into named helper methods

## Files

- `swissarmyhammer-entity/src/context.rs`

## Acceptance Criteria

- [ ] All five functions are at or below 3-level nesting
- [ ] No behavioral changes — all existing tests pass
- [ ] `code-quality:cognitive-complexity` validator passes

#refactor