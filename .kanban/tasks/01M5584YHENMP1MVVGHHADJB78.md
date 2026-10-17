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
- actor: claude-code
  id: 01m55hd9gtxk4sqt668rkm8fgn
  text: |-
    ### research
    - `hash_files` runs `fs::read` in a rayon `par_iter`, logs `warn!("Failed to read {}: {}")` and returns `None`. The file is then not in `disk_map`, and `reconcile_db_entries` deletes its row.
    - `UnreadablePaths::record(root, &ignore::Error)` already records a path and counts `entries_skipped`. `startup_cleanup` reads `entries_skipped` after the walk and hash.
    - Plan: `hash_files` takes `&mut UnreadablePaths`. The parallel step keeps the same `warn` text (public output contract). A sequential step after it wraps each read error as `ignore::Error::WithPath` and gives it to a new `record_read_error`, which records it unless the kind is `NotFound`.
    - Tests: unit tests for `record_read_error` with `walk_error(path, PermissionDenied)` and `walk_error(path, NotFound)` (each user). One real mode-000 file test through `startup_cleanup` that asserts for root and for non-root, with a `make_unreadable_file` helper that shares one body with `make_unreadable_dir` and returns `LockedDir`.
  timestamp: 2026-10-17T18:18:57.946024+00:00
- actor: claude-code
  id: 01m55hw5dws651j9nbbc7xvadp
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-code-context/src/cleanup.rs. `hash_files` now takes `&mut UnreadablePaths`; new `hash_file` (same `warn!("Failed to read {}: {}")` text) and `record_read_error` (records each read error except `NotFound`, so the row stays and `entries_skipped` counts it). New tests: `test_reconcile_keeps_row_of_file_with_denied_read` and `test_not_found_read_error_records_nothing` (constructed errors with `walk_error`, run for each user), `test_cleanup_keeps_row_of_unreadable_file` (real mode 000 file, asserts for root and non-root, no early return). Test helpers: `lock_path` shared by `make_unreadable_dir` and new `make_unreadable_file` (both give `LockedDir`), `assert_locked_row_kept` shared by the two mode-000 tests, `seed_rows` shared by the two reconcile tests. RED seen: the mode-000 test failed with "the row of the unreadable file must stay" before the fix. `cargo nextest run -p swissarmyhammer-code-context`: 766 passed, 0 skipped. `cargo nextest run -E 'rdeps(swissarmyhammer-code-context)'`: 5048 passed, 0 skipped. clippy `-D warnings` clean, `cargo fmt --all -- --check` clean.
    - note: the `info!` text "skipped {} walk entries that could not be read" is not changed (public output contract); the count now also has the files that the hash could not read.
    - next: /review
  timestamp: 2026-10-17T18:27:05.276203+00:00
- actor: claude-code
  id: 01m55j4b0a07rvjbaf08c4p8x3
  text: |-
    ### test - green
    - evidence: cargo nextest run --workspace -E 'rdeps(swissarmyhammer-code-context)' --no-fail-fast - 5048 run, 5048 passed, 0 failed, 0 skipped (65 slow, 1 leaky). cargo clippy -p swissarmyhammer-code-context --all-targets -- -D warnings: clean. cargo fmt --check: clean.
    - known: 1 LEAK marker on swissarmyhammer-tools git_diff_integration_test test_function_renamed. It is not in cleanup.rs. It is the known leak marker ^ts2s5jt. No FSEvents or Dart failures occurred.
    - next: review
  timestamp: 2026-10-17T18:31:33.130454+00:00
position_column: doing
position_ordinal: '8480'
title: Do not delete a row when the cleanup hash cannot read its file
---
## Problem
In `crates/swissarmyhammer-code-context/src/cleanup.rs`, `hash_files` logs a `warn` and drops a file when `fs::read` fails. The file is then not in the walk result, and `reconcile_db_entries` deletes its row. A temporary read error (for example, permission denied on one file, mode 000) thus deletes a valid index row. ^7ps6xar fixed the same problem for walk error entries only, with `UnreadablePaths`.

## Fix
- When `fs::read` fails with an error other than `NotFound`, record the path in the same `UnreadablePaths` set, so that the reconcile keeps the row.
- Count the entry in `CleanupStats::entries_skipped`.

## Tests
- A unit test with a file of mode 000 (skip when the process runs as root, restore the mode in a drop guard): the seeded row for that file stays. #lsp-fix #bug