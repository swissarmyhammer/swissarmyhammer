---
name: plan
description: Plan Mode workflow. Use this skill whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban cards as the plan artifact.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/detected-projects" %}
{% include "_partials/coding-standards" %}
{% include "_partials/test-driven-development" %}

# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work.

## Goals

1. **Understand the work** — research the codebase deeply enough to know what needs to change and what will be affected.
2. **Produce a kanban board** — the plan artifact is kanban cards with subtasks. Not a markdown document, not built-in task tools (TodoWrite, TaskCreate, TaskUpdate).
3. **Right-size the cards** — each card is a single focused unit of work that can be independently implemented and verified.
4. **Collaborate with the user** — present cards, discuss, iterate, and refine until the user is satisfied with the plan.
5. **Hand off cleanly** — when planning is complete, remind the user they can execute with `/implement-loop` (autonomous) or `/implement` (one card at a time).

## Constraints

### Plans are kanban cards
Every planned work item becomes a kanban card. The kanban board IS the plan. No markdown plan files. When presenting the plan, show the cards.

### Research before cards
Use `code_context` as the primary research tool. Always check blast radius (`op: "get blastradius"`) on files you expect to change — this is how you discover downstream work you'd otherwise miss. Use symbol search, call graphs, and text search (Glob/Grep/Read) to fill in the picture.

### Every card must be actionable
Card descriptions MUST include:

```
## What
<what to implement — affected files, approach, context>

## Acceptance Criteria
- [ ] <observable outcome that proves the work is done>

## Tests
- [ ] <specific test to write or update, with file path>
- [ ] <test command to run and expected result>
```

A card without acceptance criteria and tests is not a valid card. Include enough context that someone reading only the card (not the spec) can implement it.

### Card sizing limits

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 generated or modified | > 500 lines |
| Files touched | 2–4 files | > 5 files |
| Subtasks | 3–5 per card | > 5 subtasks |
| Concerns | 1 per card | Multiple distinct concerns |

The subtask cap is the most important constraint. More than 5 subtasks means the card bundles multiple concerns — split along natural seams (different files, layers, or concerns) and link with `depends_on`. Two small cards with a dependency beat one mega-card.

### Subtasks are checklist items in the description
Subtasks go in the card's `description` as GFM checklists (`- [ ]` items). There is no separate "add subtask" API.

### Board naming
Name the board for the workspace/repository, not the specific feature being planned.

### User controls plan mode exit
Do NOT call ExitPlanMode yourself. The user decides when the plan is ready.

### No auto-implementation on exit
When the user exits plan mode or approves the plan, do NOT begin implementing. Instead, remind them:
- Use `/implement-loop` to implement all cards autonomously
- Use `/implement` to implement one card at a time

### Ordering
Foundational changes come first (data models, types, configuration), then core logic, then integration, then tests, then cleanup. Use `depends_on` to encode ordering constraints between cards.

### Specificity
Use specific file paths, function names, and type names — not vague descriptions. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" not "improve error handling."

## Autonomous Agent Mode

When operating as an autonomous agent (no Plan Mode UI), follow the `PLANNING_GUIDE.md` resource file bundled with this skill.

## Updating an Existing Plan

Update kanban cards directly — add new cards, update existing ones with `op: "update task"`, remove obsolete ones with `op: "delete task"`, and reorder dependencies as needed. The board is a living document.
