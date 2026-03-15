---
depends_on:
- 01KKF4XH1HH7S14DYTYXSPZ3N5
- 01KKF4Y930C9DWTYJC8NF0X57C
position_column: done
position_ordinal: ed80
title: Wire semantic diffs into review flow for range-based reviews
---
**Files**: `builtin/skills/review/SKILL.md`, possibly `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Problem

When reviewing a range of commits (e.g. `HEAD~3..HEAD`), the review agent needs to know which refs to pass to `get diff` for semantic diffs. Currently the skill reads full file contents but doesn't instruct the agent to use semantic diffs for range-based reviews.

## Solution

When `get changes` returns files from a range:
1. The response should include the range endpoints (start_ref, end_ref) so the agent knows what to diff against
2. The review skill should instruct the agent to call `get diff` with `left: "file@<start_ref>"` and `right: "file@<end_ref>"` for each changed file
3. The semantic diff output (added/modified/deleted/moved entities) feeds directly into the layered review — it's much richer than reading raw files

### Changes to `GitChangesResponse`

Add optional fields:
- `range: Option<String>` — the range that was used (e.g. `HEAD~3..HEAD`)  

The agent uses these to construct `get diff` calls.

### Changes to review skill

Add guidance after step 1: "When reviewing a range, use `get diff` with `file@ref` syntax to get semantic diffs. These show entity-level changes (functions added, modified, deleted) which are more useful than reading full files." #review-on-main