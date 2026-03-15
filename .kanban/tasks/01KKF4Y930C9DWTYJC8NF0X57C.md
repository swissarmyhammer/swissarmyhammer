---
depends_on:
- 01KKF4XH1HH7S14DYTYXSPZ3N5
position_column: done
position_ordinal: c9
title: Update review skill to handle main-branch workflow
---
**File**: `builtin/skills/review/SKILL.md`

## Problem

Step 1 of the review skill says "get changes" but doesn't tell the agent what to do when on main with no changes. The agent gets an empty file list and produces a useless review.

## Solution

Update Step 1 ("Get the Changes") to include main-branch logic:

1. Call `git get changes` as before
2. If the result has files, proceed normally
3. If on main with no files returned:
   - Default: pass `range: "HEAD~1..HEAD"` to review the last commit
   - If the user specified a number of commits (e.g. "review last 3 commits"), the agent constructs `HEAD~3..HEAD`
   - If the user specified a range or SHAs, pass them through as `range`
4. When a `range` is used, note the range in the summary so the user knows what was reviewed

Also update the `get diff` usage: when reviewing a range, use `file@<start-ref>` and `file@<end-ref>` (or `file` for HEAD) to get semantic diffs for each changed file.

### Key point

The agent already knows git well enough to resolve SHAs, construct ranges, etc. The skill just needs to tell it about the `range` parameter and the main-branch default. #review-on-main