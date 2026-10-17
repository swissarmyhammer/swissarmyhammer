---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m5598fyhc7k36mysfjhy2qsw
  text: |-
    ### note — test rule from the ^7ps6xar review
    - Do not skip the test or return early when the process runs as root. The review rule `test-integrity/no-test-cheating` flagged this in ^7ps6xar (cleanup.rs:887).
    - Use the same design as ^7ps6xar commit 1eb198fbc: test the logic with a constructed read error (PermissionDenied) that each user can run, and make the real mode-000 test assert in both cases (non-root: the row stays and the entry is counted; root: the file is read).
    - Use the shared `walk_error` helper and the `LockedDir` helper in cleanup.rs tests.
  timestamp: 2026-10-17T15:56:32.081350+00:00
position_column: todo
position_ordinal: ffff8980
title: Do not delete a row when the cleanup hash cannot read its file
---
## Problem
In `crates/swissarmyhammer-code-context/src/cleanup.rs`, `hash_files` logs a `warn` and drops a file when `fs::read` fails. The file is then not in the walk result, and `reconcile_db_entries` deletes its row. A temporary read error (for example, permission denied on one file, mode 000) thus deletes a valid index row. ^7ps6xar fixed the same problem for walk error entries only, with `UnreadablePaths`.

## Fix
- When `fs::read` fails with an error other than `NotFound`, record the path in the same `UnreadablePaths` set, so that the reconcile keeps the row.
- Count the entry in `CleanupStats::entries_skipped`.

## Tests
- A unit test with a file of mode 000 (skip when the process runs as root, restore the mode in a drop guard): the seeded row for that file stays. #lsp-fix #bug