---
assignees:
- claude-code
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