---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m54kmv8xtpxnzp1513wpknrt
  text: 'Research: the delete behavior already exists. `startup_cleanup` builds `disk_map` from `walk_and_hash`, and `walk_and_hash` uses `workspace_walk_builder` (the shared ignore rules from ^7xn651m). `reconcile_db_entries` deletes each row that is not in `disk_map`. Thus a row for an ignored file is deleted also when the file is on disk. This agrees with the field data (FoundationModelsKanban: 102,219 rows to 253 rows). Gap: no log tells the count of removed ignored rows. These rows are counted in `files_removed` together with files that are not on disk. `workspace.rs` does not log the stats; only the periodic reconcile in swissarmyhammer-tools `watcher.rs` logs them. Plan: add `CleanupStats::files_ignored` (a row deleted while its file is on disk), log it from `startup_cleanup`, and add the acceptance tests (`.build/` file on disk, and a gitignored directory rule). No second set of rules.'
  timestamp: 2026-10-17T09:38:48.221497+00:00
- actor: claude-code
  id: 01m54kqhjjaqjvn19f53x1g4rm
  text: |-
    ### implement — changed
    - evidence: crates/swissarmyhammer-code-context/src/cleanup.rs. New field `CleanupStats::files_ignored`. New helper `delete_unwalked_row`: it deletes a row that the walk did not give. When the file is still on disk, the row counts as ignored; otherwise it counts as removed. `startup_cleanup` logs `code-context cleanup: removed {n} rows for files that the ignore filter skips` at info level when n > 0. New tests: `test_cleanup_removes_ignored_row_for_file_on_disk` (a real `.build/checkouts/X/a.swift` file with a `.gitignore` of `.build/`; the seeded row is deleted, and the `Sources/b.swift` row stays) and `test_cleanup_removes_row_under_gitignored_directory_rule` (`gen-out/`). RED: compile error, because `files_ignored` was missing. GREEN: 2/2. `cargo nextest run -p swissarmyhammer-code-context`: 729 passed, 0 skipped. `cargo clippy -p swissarmyhammer-code-context --all-targets -- -D warnings`: clean. `cargo fmt`: applied. The code uses no second set of rules; the walk is still the only filter.
    - note: rows for ignored files are now counted in `files_ignored`, not in `files_removed`. The periodic reconcile log line in swissarmyhammer-tools `watcher.rs` is not changed; the new info line from `startup_cleanup` gives the ignored count for both the startup path and the periodic path.
    - next: /review
  timestamp: 2026-10-17T09:40:16.595012+00:00
- actor: claude-code
  id: 01m54m17vjseka8erbwmxnc8vt
  text: |-
    ### test — green
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-code-context)'` ran 4984 tests. 4984 passed, 0 failed, 0 skipped (64 slow, 1 leaky). `cargo clippy --workspace --all-targets -- -D warnings` shows no warnings (exit 0). `cargo fmt --check` shows no differences (exit 0).
    - next: review
  timestamp: 2026-10-17T09:45:34.322456+00:00
depends_on:
- 01M53GC28P974P34HEP7XN651M
position_column: doing
position_ordinal: '8480'
title: Remove ignored rows from indexed_files during reconcile
---
## Problem
Workspaces that the watcher bug affected already have many ignored rows in `.code-context/index.db`. FoundationModelsKanban has 101,972 rows under `.build/`. The fix to the watcher filter stops new rows, but the old rows stay. The LSP worker can still send these paths to the LSP while `lsp_indexed = 0`.

## Fix
- In `startup_cleanup` (`crates/swissarmyhammer-code-context/src/cleanup.rs`), delete each `indexed_files` row whose path the shared ignore filter skips. Do this also when the file is on disk. CASCADE removes the chunks, symbols and edges.
- Use the shared filter from the watcher-filter task. Do not make a second set of rules.
- Log the count of removed ignored rows.

## Acceptance criteria
- A DB with a row for `.build/checkouts/X/a.swift` (the file is on disk) has no such row after `startup_cleanup`.
- A row for a normal source file stays.

## Tests
- A unit test in `cleanup.rs` with a temp workspace, a `.gitignore` with `.build/`, a real file under `.build/`, and a seeded row for it. #lsp-fix #bug