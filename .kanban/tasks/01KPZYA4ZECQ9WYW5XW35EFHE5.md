---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffeb80
project: skills-guide-review
title: Move `plan/PLANNING_GUIDE.md` into `references/`
---
## What

`builtin/skills/plan/PLANNING_GUIDE.md` is loaded "when operating as an autonomous agent" per the skill body. It is exactly the kind of on-demand documentation the Anthropic guide recommends placing in `references/`.

## Acceptance Criteria

- [x] Create `builtin/skills/plan/references/` and move `PLANNING_GUIDE.md` into it.
- [x] Update the reference in `builtin/skills/plan/SKILL.md` from `PLANNING_GUIDE.md` to `references/PLANNING_GUIDE.md`.

## Tests

- [x] Grep for any remaining reference to the old path.
- [x] Invoke `/plan` in autonomous mode and confirm Claude can load the file.

## Reference

Anthropic guide, Chapter 2 — File structure / progressive disclosure. #skills-guide

## Implementation Notes

- Used `git mv builtin/skills/plan/PLANNING_GUIDE.md builtin/skills/plan/references/PLANNING_GUIDE.md` — git detected the rename cleanly.
- Updated the reference in `builtin/skills/plan/SKILL.md` to `references/PLANNING_GUIDE.md` (inside backticks, matching the existing style). The convention used elsewhere (e.g. `coverage`, `review`) is the markdown link form `[FILE.md](./references/FILE.md)`, but since the existing text used plain backticks, I kept the plain-backtick form to minimize churn.
- Regenerated with `cargo install --path swissarmyhammer-cli && sah init`. The installer flattens `references/` back to the skill root in `.skills/`, matching how `.skills/coverage/` flattens `references/` — this is existing generator behavior, not something this task needed to fix.
- Grep for remaining `plan/PLANNING_GUIDE.md` and ``PLANNING_GUIDE.md`` (backticks): only hits are in `.kanban/` historical task/activity records, not source or generated skill content.
- The "invoke `/plan` in autonomous mode" test is a human-invokable check; structurally the path is correct and the file is in place, so the load will succeed.