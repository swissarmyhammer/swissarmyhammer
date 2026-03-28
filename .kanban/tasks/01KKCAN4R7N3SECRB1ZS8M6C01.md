---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc380
title: 'STATUSLINE-M12: git_state module'
---
## What
Implement the `git_state` module that detects in-progress git operations (rebase, merge, cherry-pick, etc.) using `git2`.

**File**: `swissarmyhammer-statusline/src/modules/git_state.rs`

**Source data**: `git2::Repository::state()` returns `RepositoryState` enum

**Default format**: `($state $progress)`

**Config**:
```yaml
git_state:
  style: "yellow bold"
  format: "($state $progress)"
```

**Variables**: `$state` (REBASING, MERGING, CHERRY-PICKING, etc.), `$progress` (e.g., "3/10" for interactive rebase)

**Detected states** (via `git2::RepositoryState`):
- `Rebase` / `RebaseInteractive` / `RebaseMerge` → "REBASING"
- `Merge` → "MERGING"
- `CherryPick` / `CherryPickSequence` → "CHERRY-PICKING"
- `Revert` / `RevertSequence` → "REVERTING"
- `Bisect` → "BISECTING"
- `ApplyMailbox` / `ApplyMailboxOrRebase` → "APPLYING"

**Progress detection**: For interactive rebase, read `.git/rebase-merge/msgnum` and `.git/rebase-merge/end` to get current/total step counts.

## Acceptance Criteria
- [ ] Uses `git2::Repository::state()`, NOT `Command::new("git")`
- [ ] Maps all RepositoryState variants to human-readable strings
- [ ] Hidden when state is `Clean` (no operation in progress)
- [ ] Reads rebase progress files when in rebase state
- [ ] Format string supports `$state` and `$progress` variables

## Tests
- [ ] Unit test: state mapping for each RepositoryState variant
- [ ] Unit test: hidden when state is Clean
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline