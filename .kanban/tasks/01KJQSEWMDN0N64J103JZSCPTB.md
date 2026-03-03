---
title: 'WARNING: read_changelog silently drops malformed JSONL lines'
position:
  column: todo
  ordinal: b3
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 216-220

**What:** `read_changelog` uses `filter_map(|line| serde_json::from_str::<ChangeEntry>(line).ok())` to parse JSONL lines. Any line that fails to parse is silently discarded. There is no logging, no error count, and no way for the caller to know that entries were lost.

**Why:** This makes debugging data corruption or schema evolution extremely difficult. If a future change modifies `ChangeEntry` (e.g., renaming a field or adding a required field), all historical entries that don't match the new schema would silently vanish. The caller would see a shorter history with no indication of data loss.

**Suggestion:** At minimum, log a warning via `tracing::warn!` for each unparseable line, including the line number and a truncated preview of the line content. Consider returning a struct that includes both parsed entries and a count of skipped lines.

- [ ] Add tracing::warn for unparseable lines with line number and preview
- [ ] Consider returning `(Vec<ChangeEntry>, usize)` where the second element is skip count
- [ ] Add a test with a deliberately malformed line to verify the warning fires #warning