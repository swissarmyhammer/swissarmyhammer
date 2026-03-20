---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc180
title: 'STATUSLINE-M10: git_branch module'
---
## What
Implement the `git_branch` module using `git2` library APIs (no shelling out).

**File**: `swissarmyhammer-statusline/src/modules/git_branch.rs`

**Source data**: `git2::Repository::discover()` then `repo.head()` to get branch name

**Default format**: ` $branch`

**Config**:
```yaml
git_branch:
  style: "purple"
  symbol: " "
  format: "$symbol$branch"
  truncation_length: 20
  truncation_symbol: "…"
```

**Variables**: `$symbol` (configurable branch icon), `$branch` (branch name, truncated)

**Example output**: ` main`, ` feature/very-long-bra…`

**API usage**:
- `git2::Repository::discover(".")` — find repo from cwd
- `repo.head()` — get HEAD reference
- `reference.shorthand()` — get branch name
- Detached HEAD: show first 7 chars of commit hash

## Acceptance Criteria
- [ ] Uses `git2` crate, NOT `Command::new("git")`
- [ ] Discovers repo from current directory
- [ ] Shows branch name from HEAD shorthand
- [ ] Handles detached HEAD (shows truncated commit hash)
- [ ] Truncates long branch names with configurable length + symbol
- [ ] Hidden when not in a git repo
- [ ] Format string supports `$symbol` and `$branch` variables

## Tests
- [ ] Unit test: extracts branch name from a test git2 repo (use `git2::Repository::init()` in tempdir)
- [ ] Unit test: truncation at configured length
- [ ] Unit test: detached HEAD shows commit hash prefix
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline