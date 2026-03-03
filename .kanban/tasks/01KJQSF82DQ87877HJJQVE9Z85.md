---
title: 'WARNING: text_diff_round_trip test only covers single-hunk substitution'
position:
  column: todo
  ordinal: b4
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 627-642

**What:** The `text_diff_round_trip` test uses a 5-line text where line 2 is changed and line 6 is appended. With `context_radius(3)`, this produces a single hunk. The test does NOT cover:
1. Multi-hunk diffs (changes far enough apart to produce separate hunks)
2. Line insertions (net line count increase)
3. Line deletions (net line count decrease)
4. Mixed insertions and deletions across hunks

**Why:** The multi-hunk reversal bug (blocker #1) was not caught because no test exercises it. The existing test only validates single-hunk substitution, which happens to work even with the broken reversal logic.

**Suggestion:** Add tests with 20+ line texts that produce 2+ hunks, including cases where insertions/deletions change the line count asymmetrically between hunks.

- [ ] Add multi-hunk round-trip test (20+ lines, changes at positions 3 and 18)
- [ ] Add test with net line insertion in first hunk + substitution in second hunk
- [ ] Add test with net line deletion in first hunk + substitution in second hunk
- [ ] Verify all round-trips: forward apply matches new, reverse apply restores old #warning