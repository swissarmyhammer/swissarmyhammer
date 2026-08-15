---
assignees:
- claude-code
position_column: todo
position_ordinal: ffe080
title: Stale comment in builtin/mod.rs claims Dart's only metrics tool is commercial
---
`crates/swissarmyhammer-validators/src/builtin/mod.rs:306-308` says Dart "keeps the `complexity` probe and both prompt rules, because its only metrics tool is commercial."

**Both clauses are now false.**

- Dart supersedes `function-length` at line 326 of that same file, shipped by `^xskz2ez`.
- `dart_code_linter` 4.2.0 is MIT and maintained — a free fork of the discontinued `dart_code_metrics` on a current analyzer. VALIDATOR.md already retracts the commercial claim.

The identical sibling comment WAS corrected, at `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs:330-336`. This second copy was missed.

## Why it was not fixed under ^xskz2ez

Found by the reviewer of `5df34d385`, and deliberately not forced into that card's checklist. The commit's entire edit to `builtin/mod.rs` is one added line; this comment is untouched context that the commit made false without rewriting. Under a diff op it is off-diff, and the engine refutes off-diff candidates before they reach the report — so the engine's silence was correct behaviour rather than a miss. Manufacturing an out-of-scope finding would have corrupted the contract `^apb04az` established.

That is the right call for the review gate, and it is exactly why this needs a card instead: a true statement made false by a neighbouring change has no other route into the work.

## What to do

- Correct the comment to match what the crate now does.
- Check whether any other copy of the same claim survives. Two were known; the sibling in `tests.rs` is already fixed.

## Done when

- No file states that Dart's only metrics tool is commercial.
- No file states that Dart keeps both prompt rules.

#tool-validators