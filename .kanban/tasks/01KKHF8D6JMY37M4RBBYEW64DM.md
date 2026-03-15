---
depends_on:
- 01KKHF803PJ5VJH5KKZCM20QD2
- 01KKHF7HYJ6QPXPBDEG6V5VK3P
position_column: done
position_ordinal: e6
title: 'SEM-4: Replace GitBridge with shell git in auto-diff'
---
## What\nThe only use of `GitBridge` (and thus git2) is in `execute_auto_diff()` in `swissarmyhammer-tools/src/mcp/tools/git/diff/mod.rs`. It calls:\n- `GitBridge::open(working_dir)` \n- `bridge.detect_and_get_files()` → returns `(DiffScope, Vec<FileChange>)`\n\nReplace this with shell git commands:\n1. `git status --porcelain` to detect changed files\n2. `git diff --name-status` for working tree changes\n3. `git diff --cached --name-status` for staged changes  \n4. `git show HEAD:<path>` for before content\n5. `fs::read_to_string` for after content (working tree)\n\nThis logic lives in swissarmyhammer-tools, NOT in swissarmyhammer-sem. The new crate provides parsing/diffing only — git integration is the consumer's responsibility.\n\nFiles:\n- `swissarmyhammer-tools/src/mcp/tools/git/diff/mod.rs` — rewrite `execute_auto_diff()` to use shell git\n\n## Acceptance Criteria\n- [ ] `execute_auto_diff` works without git2\n- [ ] Detects staged files, working tree changes\n- [ ] Populates FileChange structs with before/after content\n- [ ] No `use sem_core::git::bridge` anywhere\n\n## Tests\n- [ ] `swissarmyhammer-tools/tests/git_diff_integration_test.rs` passes\n- [ ] `swissarmyhammer-tools/tests/git_tool_integration_test.rs` passes\n- [ ] Manual test: make a change, run auto-diff, see semantic changes