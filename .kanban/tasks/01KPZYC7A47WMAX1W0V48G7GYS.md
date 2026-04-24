---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffea80
project: skills-guide-review
title: Add user trigger phrases to `plan` skill description
---
## What

Current `builtin/skills/plan/SKILL.md` description:

> Plan Mode workflow. Use this skill whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban tasks as the plan artifact.

"Whenever you are in Plan Mode" is an environment trigger, not a user phrase. Add explicit phrases users would say ("/plan", "help me plan", "break this into tasks", "design the approach").

## Acceptance Criteria

- [ ] Description includes specific user trigger phrases.
- [ ] Retains the Plan Mode trigger as one of several conditions.
- [ ] Under 1024 chars, no `<`/`>`.

## Tests

- [ ] Trigger test: "help me plan this feature" → loads `plan`.
- [ ] Trigger test: "break this down into tasks" → loads `plan`.

## Reference

Anthropic guide, Chapter 2 — "The description field". #skills-guide