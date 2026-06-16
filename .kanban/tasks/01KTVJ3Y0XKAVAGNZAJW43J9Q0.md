---
assignees:
- claude-code
position_column: todo
position_ordinal: 9c80
title: Stale deployed .skills/.claude/.zed skill copies shadow builtin and break skill_e2e short-id tests
---
## What
`cargo nextest run -p swissarmyhammer-tools` fails on two tests when the repo working tree contains the (untracked, generated) deployed skill directories:

- `integration::skill_e2e::test_kanban_skill_renders_short_id_guidance`
- `integration::skill_e2e::test_task_skill_renders_short_id_guidance`

Root cause: `.skills/kanban`, `.claude/skills/kanban`, `.zed/skills/kanban` (and the `task` siblings) in the repo root are **stale renders** produced before the `{% include "_partials/short-ids" %}` partial was added to `builtin/skills/kanban/SKILL.md` / `builtin/skills/task/SKILL.md`. The skill loader prefers the project-level deployed copies over `builtin/`, so `use skill` returns the stale body without the marker phrase `last 7 characters of the ULID`.

Verified: moving the six stale directories aside makes both tests pass; restoring them makes the tests fail again. No source change is involved.

## Possible fixes (pick one)
- Make the deploy step regenerate `.skills/` & friends whenever builtin skill sources change (staleness is the bug), or
- Make the skill_e2e tests hermetic: run with a CWD/HOME that cannot see the repo's deployed skill dirs (they already use a TempDir board — the skill lookup escapes it), or
- Have the loader prefer builtin when the deployed copy's version/hash is older than builtin.

## Acceptance Criteria
- [ ] `cargo nextest run -p swissarmyhammer-tools -E 'test(renders_short_id_guidance)'` passes with stale deployed skill dirs present in the repo root.
- [ ] No regression in skill precedence semantics (project skills still override builtin when intentionally customized).