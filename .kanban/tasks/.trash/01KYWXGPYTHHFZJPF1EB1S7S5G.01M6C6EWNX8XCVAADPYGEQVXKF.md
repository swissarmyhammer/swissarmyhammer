---
assignees:
- claude-code
position_column: todo
position_ordinal: d580
title: append_tag builds its two candidate bodies with duplicate code
---
`append_tag` in `crates/swissarmyhammer-kanban/src/tag_parser.rs` writes the marker twice, in two near-identical blocks:

- the inline candidate — clone the text, push a space when the text does not already end in whitespace, push `#slug`;
- the own-line candidate — clone the text, push a newline when the text does not already end in one, push `#slug`.

The two differ only in the separator character and in the predicate that decides whether the separator is needed. The `complexity` validator flagged the pair as parameterizable duplication.

Pre-existing. Found by re-running the validator on `tag_parser.rs` while working ^tnr56gg (a nesting-depth finding elsewhere in the same file). It is a different cause, on code ^tnr56gg did not touch, so it was split out instead of folded in.

## Required change

Fold the two blocks into one helper that takes the separator and the "already separated" predicate, then call it twice. Keep the two-step try-inline-then-own-line control flow in `append_tag` itself — that ordering is the documented behavior (`append_tag` prefers inline and falls back to its own line when the body would swallow the marker), and it is pinned by `test_append_tag_round_trips_on_bodies_that_swallow_inline_markers`.

## Acceptance

- One place writes `#slug` onto a candidate body.
- No behavior change: every existing `append_tag` test passes unchanged. A test that needs editing is a signal the behavior moved, so stop and report instead.
- Any new helper's docstring states only what it does — no claim of coverage a test does not hold.
- `cargo nextest run -p swissarmyhammer-kanban` green, `cargo clippy -p swissarmyhammer-kanban --all-targets -- -D warnings` clean, `cargo fmt --check` clean.
#chore