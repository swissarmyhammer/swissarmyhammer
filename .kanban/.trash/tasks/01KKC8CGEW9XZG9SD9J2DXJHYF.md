---
depends_on: []
position_column: todo
position_ordinal: a4
title: 'STATUSLINE-5: Tool modules (git2 starship-style, kanban bar, index, languages)'
---
## What
Implement the 6 tool modules that query sah's Rust libraries. NO shelling out — all via library APIs.

Key files:
- `swissarmyhammer-statusline/src/modules/git_branch.rs` — uses `git2::Repository::discover()` + `repo.head()` for branch name
- `swissarmyhammer-statusline/src/modules/git_status.rs` — starship-style status counts using `git2::Repository::statuses()` + `graph_ahead_behind()`. Shows modified(!), staged(+), untracked(?), deleted(✘), conflicted(=), stashed($), ahead(⇡N), behind(⇣N), diverged(⇕). All symbols configurable.
- `swissarmyhammer-statusline/src/modules/git_state.rs` — uses `git2::Repository::state()` to detect Rebase, Merge, CherryPick, Revert, Bisect, ApplyMailbox. Shows progress when available.
- `swissarmyhammer-statusline/src/modules/kanban.rs` — progress bar! Opens `.kanban/` board, counts done/total tasks, renders `📋 [████░░] 3/7` with configurable bar_width.
- `swissarmyhammer-statusline/src/modules/index.rs` — uses `CodeContextWorkspace::open()` as Reader, calls `get_status(conn)` for StatusReport. Shows `idx 85%` during indexing, hidden when complete (configurable).
- `swissarmyhammer-statusline/src/modules/languages.rs` — queries indexed_files paths for extensions, maps to language icons (🦀🐍📜🐹☕💎🐦 etc.). Checks LSP server availability via `find_executable()`. Icons dimmed when LSP not in PATH.

### git_status detail (starship parity)
All via `git2` API:
- `repo.statuses(None)` returns `StatusEntry` items with flags for index/workdir changes
- Count: `INDEX_NEW` (staged), `WT_MODIFIED` (modified), `WT_NEW` (untracked), `WT_DELETED` / `INDEX_DELETED` (deleted), `CONFLICTED` (conflicted)
- `repo.stash_foreach()` for stash count
- `repo.graph_ahead_behind(local_oid, remote_oid)` for ahead/behind

### languages detail
- Query: `SELECT DISTINCT file_path FROM indexed_files` then extract extensions
- Map extensions to languages via `LanguageRegistry::global().detect_language()`
- For each detected language, check LSP availability:
  - rust → `find_executable(\"rust-analyzer\")`
  - python → `find_executable(\"pyright\")` or `find_executable(\"pylsp\")`
  - typescript → `find_executable(\"typescript-language-server\")`
  - go → `find_executable(\"gopls\")`
  - etc.
- Render icon with full style if LSP available, dimmed if not

## Acceptance Criteria
- [ ] git_branch uses git2 API exclusively, never shells out
- [ ] git_status shows starship-style counts (modified, staged, untracked, ahead/behind)
- [ ] git_state detects rebase/merge/cherry-pick via git2::RepositoryState
- [ ] kanban renders a progress bar, not just numbers
- [ ] index reads .code-context/index.db via library API
- [ ] languages detects project languages from indexed_files and checks LSP availability
- [ ] All six return None gracefully when their data source doesn't exist

## Tests
- [ ] git_branch: test in temp git repo
- [ ] git_status: test with staged, modified, untracked files in temp repo
- [ ] git_status: test ahead/behind counts
- [ ] git_state: test normal state vs rebase state
- [ ] kanban: test with temp .kanban/ board, verify progress bar output
- [ ] index: test with mock StatusReport
- [ ] languages: test extension-to-icon mapping
- [ ] All modules: test returns None when data source absent
- [ ] `cargo test -p swissarmyhammer-statusline`"