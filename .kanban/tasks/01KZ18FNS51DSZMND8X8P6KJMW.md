---
assignees:
- claude-code
position_column: todo
position_ordinal: e480
title: blame shas in the review prime drift the prompt between finish-loop iterations
---
Found while investigating ^k5wsxh0.

`render_numbered_source` in `crates/swissarmyhammer-validators/src/review/fleet.rs` prints every source line as `{line} | {sha} {mark} | {text}`. The sha comes from libgit2 blame, computed in the scope stage.

For `Working`, `File`, and `Glob` scopes, `blame_at` is `None`, so blame binds to HEAD:

- Any commit, amend, or rebase between two runs shifts every sha.
- An uncommitted line renders `worktree`. Commit it and the same review renders a real sha, with no source byte changed.
- `git add` alone flips `untrackd` to `worktree`, because `path_is_tracked` consults the index when `at` is `None`.
- A failed blame degrades to `????????` for every line of that file, changing the prompt silently rather than erroring.

Only `Scope::Sha` pins blame.

## Why it matters

Two back-to-back runs on an unchanged worktree are unaffected. But the finish loop commits between iterations, so **every iteration sends a different prompt for unchanged code**. That breaks prefix-cache reuse, and it feeds the stuck-detection guardrail a moving target — the guardrail declares a task stuck when the SAME finding survives 3 iterations.

## Work

Decide what the blame column is for and make it stable against it. Options to weigh:

- Pin `blame_at` to the merge-base or to the review's base revision for working scopes, so the column answers "who last touched this before my change" rather than "what does HEAD say right now".
- Or drop the sha column and keep only the touched-mark, if the sha is not carrying its weight in prompt bytes.

Do not simply widen a tolerance. The column must mean one thing and mean it every run.

## Acceptance

- Two `review working` runs across an intervening commit of unrelated files render byte-identical prompts for a file neither run changed.
- A test pins it.

#bug #review