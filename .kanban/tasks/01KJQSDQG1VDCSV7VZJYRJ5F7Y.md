---
title: 'BLOCKER: reverse_unified_diff produces malformed headers'
position:
  column: todo
  ordinal: b0
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 240-253

**What:** The header reversal logic in `reverse_unified_diff` produces malformed output. Input `"--- old"` becomes `"+++-- old"` (not `"+++ old"`), and `"+++ new"` becomes `"---++ new"` (not `"--- new"`). This is because `strip_prefix('-')` on `"--- old"` yields `"-- old"`, and then `format!("+++{}", "-- old")` concatenates the remaining dashes.

**Why:** Although `apply_unified_diff` happens to tolerate this because its header skip loop only checks `starts_with("---") || starts_with("+++")`, the output is not valid unified diff format. If the reversed diff is ever consumed by an external tool (e.g., `patch`, `git apply`, or stored for human inspection), it will be rejected or confusing. The header order is also wrong: the reversed diff should have `--- new` before `+++ old`, but the current code preserves the original order.

**Suggestion:** Replace the line-by-line prefix swap with proper header handling. Detect the `---`/`+++` header pair, extract the filenames, and emit them swapped: `--- <new_name>` then `+++ <old_name>`.

- [ ] Fix header reversal to produce `--- new` / `+++ old` (not `+++-- old` / `---++ new`)
- [ ] Verify reversed diffs are valid unified diff format
- [ ] Add a unit test that checks reversed header output explicitly #blocker