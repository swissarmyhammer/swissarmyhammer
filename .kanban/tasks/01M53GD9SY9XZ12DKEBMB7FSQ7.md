---
assignees:
- claude-code
depends_on:
- 01M53GC28P974P34HEP7XN651M
position_column: todo
position_ordinal: fffd80
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