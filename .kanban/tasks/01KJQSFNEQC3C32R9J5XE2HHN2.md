---
title: 'WARNING: consider replacing custom diff apply/reverse with similar crate'
position:
  column: todo
  ordinal: b5
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 236-354 (all of reverse_unified_diff, apply_unified_diff, parse_hunk_header)

**What:** The codebase uses `similar` to CREATE diffs but then implements custom `apply_unified_diff` and `reverse_unified_diff` functions by hand-parsing unified diff format. This is roughly 120 lines of subtle, bug-prone code that reimplements functionality available in the `similar` crate itself.

**Why:** `similar` already provides `TextDiff::from_lines()` which can produce diffs in either direction. Instead of reversing a diff, you can simply call `make_text_diff(new, old)` to get the reverse diff directly. For applying diffs, you could store old/new text pairs or use `similar`'s change operations directly rather than parsing unified diff text.

This would eliminate all three blockers above (hunk header reversal, malformed headers, trailing newline handling) because you'd never need to parse or reverse unified diff text.

**Suggestion:** Consider one of:
1. Store `similar::ChangeTag` operations instead of unified diff text strings
2. For reversal, call `make_text_diff(new_text, old_text)` instead of `reverse_unified_diff`
3. If unified diff text must be stored (for human readability in JSONL), keep the current `make_text_diff` for storage but use `similar` directly for application

- [ ] Evaluate whether `similar`'s API supports direct patch application
- [ ] If not, consider storing old+new text hashes alongside the diff for verification
- [ ] Prototype the `make_text_diff(new, old)` approach for reversal to avoid custom reversal entirely #warning