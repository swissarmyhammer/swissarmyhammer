---
assignees:
- claude-code
depends_on:
- 01KT6SAAM6CR85YZD26JHSC87E
position_column: todo
position_ordinal: '8780'
project: short-ids
title: 'Short IDs: document short ids in kanban + task skills'
---
Update the kanban-facing skills so agents understand and use short ids instead of inventing ULID prefix abbreviations.

## Why
Agents instinctively abbreviate task ULIDs by PREFIX (e.g. `01KT6SA4`), which clusters and near-collides for tasks created in the same session. The skills should teach the agent to quote the canonical short id (`^<7char>` suffix) that now appears in tool output, and explain how task references resolve.

## Files (edit SOURCE, not generated)
- `builtin/skills/kanban/SKILL.md`
- `builtin/skills/task/SKILL.md`
- NOTE: `.skills/` is generated — never edit there; edit `builtin/skills/<name>/SKILL.md`. Regenerate if the build requires it.
- Check `builtin/skills/implement/SKILL.md` and `builtin/skills/finish/SKILL.md` too — anywhere that tells the agent to reference a task by id should mention the short id.

## Content to add
- What a short id is: lowercase last-7 Crockford chars of the ULID, shown as `^<short>` (e.g. `^rc9rb4g`). It is the canonical short handle; the full 26-char ULID stays the stored identity.
- Prefer quoting the short id (from the tool's `short_id` field) when referring to a task in prose/commits/chat — do NOT hand-abbreviate the ULID by prefix.
- How references resolve (forgiving input): full ULID, 7-char short id, `^<short>`, or a unique ULID prefix all resolve to the task; an ambiguous prefix errors. Display is always the short form.
- One concrete example showing the same task by full ULID and by `^<short>`.

## Acceptance
- kanban + task skills (and implement/finish where relevant) describe short ids and the "quote short id, don't prefix-abbreviate" guidance.
- Edits are in `builtin/skills/...`, not `.skills/`.

Depends on the tool/CLI API card (short ids must actually appear in output + resolve before the docs describe them).