---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffff8c80
title: 'STATUSLINE-M8: worktree module'
---
## What
Implement the `worktree` module that displays the active worktree branch name.

**File**: `swissarmyhammer-statusline/src/modules/worktree.rs`

**Source data**: `worktree.branch` from stdin JSON

**Default format**: `🌲 $branch`

**Config**:
```yaml
worktree:
  style: "green"
  format: "🌲 $branch"
```

**Variables**: `$branch` (worktree branch name)

**Example output**: `🌲 feature-xyz`

## Acceptance Criteria
- [ ] Module reads `worktree.branch` from parsed input
- [ ] Hidden when worktree object is absent (no active worktree)
- [ ] Format string supports `$branch` variable

## Tests
- [ ] Unit test: displays branch with tree emoji
- [ ] Unit test: hidden when worktree field absent
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline