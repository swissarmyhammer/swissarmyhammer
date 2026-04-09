---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd480
title: Changelog append is not atomic -- partial writes corrupt the log
---
**swissarmyhammer-store/src/changelog.rs:68-79**\n\n`append()` opens the file in append mode and writes a JSON line directly. If the process crashes mid-write, the last line will be a partial JSON string. While `read_all()` gracefully skips unparseable lines (good), the partial line means data loss of the last operation — and no warning is emitted.\n\nMore critically, `writeln!` may split across multiple OS write calls for large entries (entries with full before/after text can be many KB). On POSIX `O_APPEND` guarantees atomicity only up to `PIPE_BUF` (typically 4096 bytes). A large changelog entry can produce a corrupt interleave if two processes append concurrently.\n\n**Severity: warning**\n\n**Suggestion:**\n1. Serialize the line to a `String` first (already done), then write it in a single `write_all` call.\n2. Consider using `File::sync_data()` after the write for durability.\n3. Log a warning when `read_all()` skips a corrupt line so silent data loss is visible.\n\n**Subtasks:**\n- [ ] Add `tracing::warn!` when a corrupt/unparseable line is skipped in `read_all`\n- [ ] Consider `fsync` after append for crash safety\n- [ ] Verify large entries (>4KB) serialize and append correctly" #review-finding