---
assignees:
- claude-code
position_column: todo
position_ordinal: f480
title: Fix flaky test_file_watcher_start_watching_sets_up_debouncer — early-return path not handled
---
**Failing test:** `swissarmyhammer-tools::mcp::file_watcher::tests::test_file_watcher_start_watching_sets_up_debouncer`

**File:** `swissarmyhammer-tools/src/mcp/file_watcher.rs` (assertion at approx the `assert!(watcher.debouncer.is_some())` line inside the test)

**Failure:**
```
thread 'mcp::file_watcher::tests::test_file_watcher_start_watching_sets_up_debouncer' panicked at swissarmyhammer-tools/src/mcp/file_watcher.rs:909:13:
assertion failed: watcher.debouncer.is_some()
```

**Root cause:** `FileWatcher::start_watching` has an early-return path when `PromptResolver::get_prompt_directories()` returns an empty list — it logs a warning and returns `Ok(())` without ever setting `self.debouncer`. The test comment claims "we test both code paths" but the `is_ok()` branch unconditionally asserts the debouncer is `Some`, which is only true when there was at least one directory to watch.

In environments where no prompt directories exist (or the serial CWD guard lands the test somewhere without any), the test fails deterministically.

**What I tried:** Ran the test in isolation — fails every time in this worktree. Pre-existing on `main`; not caused by the branch's `builtin/skills/*/SKILL.md` edits and `references/` reorg. The file was not modified on this branch.

**Acceptance criteria:**
- Test passes regardless of whether `get_prompt_directories()` returns an empty list.
- Either assert the postcondition that matches the empty-directories path (`debouncer.is_none()`), or arrange prompt directories before calling `start_watching` so the success path is deterministic.

**Tests:** `cargo nextest run --package swissarmyhammer-tools -E 'test(test_file_watcher_start_watching_sets_up_debouncer)'` passes. #test-failure