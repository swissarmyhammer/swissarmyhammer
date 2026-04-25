---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe280
project: skills-guide-review
title: Narrow `implement` skill description — over-broad trigger
---
## What

Current description of `builtin/skills/implement/SKILL.md`:

> Implementation workflow. Use this skill whenever you are implementing, coding, or building. Picks up one kanban task and does the work. Produces verbose output — automatically delegates to an implementer subagent.

"whenever you are implementing, coding, or building" is a trigger on essentially every coding request. Per the guide's over-triggering playbook, descriptions should be "more specific" and "clarify scope". This skill is specifically a **kanban task executor** — that should be the primary trigger, not generic coding.

## Acceptance Criteria

- [x] Description makes clear the skill is for executing a kanban task (not arbitrary code writing).
- [x] Includes specific trigger phrases: "/implement", "implement task", "work the next task", "pick up a task", "implement <task-id>".
- [x] Clarifies scope — e.g. "Do NOT use for free-form edits unrelated to a kanban task".
- [x] Under 1024 chars, no `<` / `>`.

## Tests

- [x] Trigger test: "fix this typo" should NOT load `implement`.
- [x] Trigger test: "/implement" or "pick up the next task" SHOULD load `implement`.
- [x] Ask Claude "when would you use the implement skill?" — description surfaces the kanban intent, not generic coding.

## Reference

Anthropic guide, Chapter 5 — over-triggering solutions (negative triggers, more specific, clarify scope).

## Resolution

Updated `builtin/skills/implement/SKILL.md` description to:

> Kanban task executor. Use this skill when the user says "/implement", "implement task", "implement the next task", "work the next task", "pick up a task", or "implement" followed by a task id. Picks up one kanban task and drives it from ready through doing to review. Produces verbose output — automatically delegates to an implementer subagent. Do NOT use this skill for free-form edits, typo fixes, refactors, or any coding work that is not tied to a specific kanban task — those are not "implementation" in this skill sense. If there is no kanban task yet, use the `task` or `plan` skill to create one first.

- 615 chars (well under 1024).
- No `<` / `>` characters in the description field.
- Leads with "Kanban task executor" to make the scope immediately obvious.
- Lists concrete trigger phrases per the acceptance criteria.
- Adds an explicit negative-trigger clause to prevent misfire on typo fixes / refactors / free-form edits.
- Points users toward `task` / `plan` skills when no kanban task exists yet. #skills-guide