---
title: 'BLOCKER: reverse_unified_diff does not swap hunk header line numbers'
position:
  column: todo
  ordinal: a9
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 237-260

**What:** `reverse_unified_diff` swaps `+`/`-` prefixes on content lines and attempts to swap headers, but it does NOT modify hunk header line numbers (`@@ -old_start,count +new_start,count @@`). When a forward diff inserts or deletes lines, the old_start and new_start in hunk 2+ become asymmetric. The reversed diff needs those numbers swapped (`-old` becomes `-new` and vice versa), but they are passed through unchanged.

**Why:** When `apply_unified_diff` processes the reversed diff's second hunk, it uses the unreversed `old_start` to seek in the (now-swapped) "old" text. This causes it to skip or duplicate lines between hunks. Confirmed: a 20-line file with an insertion at line 3 and a substitution at line 18 produces a corrupted restore with lines 13-14 missing and lines 19-20 duplicated.

**Suggestion:** In `reverse_unified_diff`, parse `@@ -A,B +C,D @@` headers and emit `@@ -C,D +A,B @@`. Alternatively, avoid custom diff reversal entirely and use `similar` to produce a fresh diff from new-to-old.

- [ ] Fix hunk header swapping in `reverse_unified_diff`
- [ ] Add a multi-hunk test where forward diff inserts/deletes lines (not just substitutions)
- [ ] Verify round-trip: diff, reverse, apply restores original for insertion/deletion cases
- [ ] Run full test suite to confirm no regressions #blocker