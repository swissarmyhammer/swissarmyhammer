---
title: 'BLOCKER: apply_unified_diff mishandles trailing newline differences'
position:
  column: todo
  ordinal: b1
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 266-329

**What:** `apply_unified_diff` does not handle the case where old and new text differ in trailing newline. The final newline logic (lines 324-327) only preserves a trailing newline if the OLD text had one. But the diff may be adding or removing a trailing newline. The `\\ No newline at end of file` marker in unified diffs encodes this information, but `apply_unified_diff` treats it as an unknown line and skips it.

**Confirmed:** 
- `apply_unified_diff("abc", diff_that_adds_trailing_newline)` produces `"xyz"` instead of `"xyz\\n"` (forward apply fails)
- `apply_unified_diff("abc\\n", diff_that_removes_trailing_newline)` produces `"xyz\\n"` instead of `"xyz"` (forward apply fails)
- Round-trip reversal also fails in both cases

**Why:** While kanban entity fields are typically short strings without trailing newline concerns, this is a general-purpose diff apply function. Incorrect handling means any string field that gains or loses a trailing newline will silently produce wrong results.

**Suggestion:** Parse `\\ No newline at end of file` markers to determine whether the result should end with a newline. Track which side (old/new) the marker refers to based on whether it follows a `-` or `+` line.

- [ ] Handle `\\ No newline at end of file` markers in `apply_unified_diff`
- [ ] Add tests for: old has newline + new doesn't, old doesn't + new has newline
- [ ] Verify round-trip for trailing newline changes #blocker