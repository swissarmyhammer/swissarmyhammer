---
title: 'WARNING: no bounds on JSONL changelog file growth'
position:
  column: todo
  ordinal: b6
---
`/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-entity/src/changelog.rs` lines 189-223

**What:** `append_changelog` appends indefinitely to a JSONL file, and `read_changelog` reads the entire file into memory as a single `String`. There is no rotation, truncation, compaction, or size limit.

**Why:** For a kanban board with moderate activity this is fine. But for a long-lived board with thousands of task updates, changelog files could grow to megabytes. The `read_to_string` call loads the entire file, and every line is parsed. This is a performance concern for read-heavy access patterns (e.g., computing an entity's full history on every request).

**Suggestion:** This is not urgent but worth noting for future work:
- Add an optional `limit` parameter to `read_changelog` to read only the last N entries
- Consider periodic compaction: snapshot the entity state and truncate the log
- Document the expected growth characteristics

- [ ] Document expected changelog growth in module docs
- [ ] Consider adding a `limit` or `since_timestamp` parameter to `read_changelog` #warning