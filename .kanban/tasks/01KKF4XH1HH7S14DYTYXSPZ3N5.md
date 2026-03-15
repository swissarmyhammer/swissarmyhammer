---
position_column: done
position_ordinal: e380
title: Add `range` parameter to `get changes` git tool
---
**File**: `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Problem

On main branch with a clean working tree, `get changes` returns an empty file list because it only falls back to `get_uncommitted_changes()`. There's no way to ask for committed changes on main.

## Solution

Add an optional `range` parameter to `GitChangesRequest` that accepts any git revision range (e.g. `HEAD~1`, `HEAD~3..HEAD`, `abc123..def456`).

### Behavior matrix

| State | Behavior |
|-------|----------|
| Feature branch (has parent) | Current behavior (diff from parent + uncommitted) |
| Main + uncommitted changes | Uncommitted changes (current, works) |
| Main + clean + no `range` | Default to `HEAD~1..HEAD` (last commit) |
| Main + clean + `range` specified | Use the range |
| Any branch + `range` specified | Range takes precedence over parent-branch detection |

### Implementation

1. Add `range: Option<String>` to `GitChangesRequest`
2. Add `range` to `GET_CHANGES_PARAMS` 
3. In `execute()`, if `range` is provided, call a new `get_changed_files_from_range()` on `GitOperations`
4. If on main with clean tree and no range, default to `HEAD~1..HEAD`
5. Parse range: if it contains `..`, split into two refs; if single ref, treat as `ref..HEAD`

### Tests

- Range `HEAD~1..HEAD` returns files from last commit
- Range `HEAD~3..HEAD` returns files from last 3 commits  
- Single ref `HEAD~2` treated as `HEAD~2..HEAD`
- Range takes precedence over parent branch detection
- Default-to-last-commit on clean main #review-on-main