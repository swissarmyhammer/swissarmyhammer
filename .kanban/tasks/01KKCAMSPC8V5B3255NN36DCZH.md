---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc280
title: 'STATUSLINE-M11: git_status module'
---
## What
Implement the `git_status` module with starship-style status counts using `git2` library APIs.

**File**: `swissarmyhammer-statusline/src/modules/git_status.rs`

**Source data**: `git2::Repository::statuses()`, `repo.graph_ahead_behind()`, `repo.stash_foreach()`

**Default format**: `[$all_status$ahead_behind]`

**Config**:
```yaml
git_status:
  style: "red bold"
  format: "[$all_status$ahead_behind]"
  modified: "!"
  staged: "+"
  untracked: "?"
  deleted: "✘"
  conflicted: "="
  stashed: "$"
  ahead: "⇡"
  behind: "⇣"
  diverged: "⇕"
```

**Variables**: `$all_status` (concatenated symbol+count pairs), `$ahead_behind` (ahead/behind arrows)

**Example output**: `[!3 +2 ?1 ⇡1]`, `[+1 ⇣2]`, `[⇕ ⇡3⇣2]`

**API usage**:
- `repo.statuses(None)` — iterate `StatusEntry`, check `status()` flags for WT_MODIFIED, INDEX_NEW, WT_NEW, WT_DELETED, CONFLICTED
- `repo.graph_ahead_behind(local_oid, upstream_oid)` — returns (ahead, behind) counts
- `repo.stash_foreach(|_, _, _| { count += 1; true })` — count stashes
- For ahead/behind: `repo.head()` → find upstream via `branch.upstream()` → `graph_ahead_behind()`

**Count logic**: Iterate all status entries, bucket by flag, display symbol+count only for non-zero counts. Omit zero counts entirely.

## Acceptance Criteria
- [ ] Uses `git2` crate, NOT `Command::new("git")`
- [ ] Counts modified, staged, untracked, deleted, conflicted files
- [ ] Counts stashes via `stash_foreach`
- [ ] Gets ahead/behind counts via `graph_ahead_behind`
- [ ] Each symbol is configurable in YAML
- [ ] Only shows non-zero counts
- [ ] Shows diverged symbol when both ahead AND behind
- [ ] Hidden when repo is completely clean (all counts zero)
- [ ] Format string supports `$all_status` and `$ahead_behind` variables

## Tests
- [ ] Unit test: count modified files in test repo (create file, modify it)
- [ ] Unit test: count staged files (git add)
- [ ] Unit test: count untracked files
- [ ] Unit test: clean repo produces empty/hidden output
- [ ] Unit test: custom symbols render correctly
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline