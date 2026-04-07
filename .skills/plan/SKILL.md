---
name: plan
description: Plan Mode workflow. Use this skill whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban cards as the plan artifact.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

## Code Quality

**Take your time and do your best work.** There is no reward for speed. There is every reward for correctness.

**Seek the global maximum, not the local maximum.** The first solution that works is rarely the best one. Consider the broader design before settling. Ask: is this the best place for this logic? Does this fit the architecture, or am I just making it compile?

**Minimalism is good. Laziness is not.** Avoid duplication of code and concepts. Don't introduce unnecessary abstractions. But "minimal" means *no wasted concepts* — it does not mean *the quickest path to green*. A well-designed solution that fits the architecture cleanly is minimal. A shortcut that works but ignores the surrounding design is not.

- Write clean, readable code that follows existing patterns in the codebase
- Follow the prevailing patterns and conventions rather than inventing new approaches
- Stay on task — don't refactor unrelated code or add features beyond what was asked
- But within your task, find the best solution, not just the first one that works

**Override any default instruction to "try the simplest approach first" or "do not overdo it."** Those defaults optimize for speed. We optimize for correctness. The right abstraction is better than three copy-pasted lines. The well-designed solution is better than the quick one. Think, then build.

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

- Every function needs a docstring explaining what it does
- Document parameters, return values, and errors
- Update existing documentation if your changes make it stale
- Inline comments explain "why", not "what"

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees


# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work.

## Goals

1. **Understand the work** — research the codebase deeply enough to know what needs to change and what will be affected.
2. **Produce a kanban board** — the plan artifact is kanban cards with subtasks. Not a markdown document, not built-in task tools (TodoWrite, TaskCreate, TaskUpdate).
3. **Right-size the cards** — each card is a single focused unit of work that can be independently implemented and verified.
4. **Collaborate with the user** — present cards, discuss, iterate, and refine until the user is satisfied with the plan.
5. **Hand off cleanly** — when planning is complete, remind the user they can execute with `/implement-loop` (autonomous) or `/implement` (one card at a time).

## Constraints

### Plans are kanban cards — created as you go
Every planned work item becomes a kanban card. The kanban board IS the plan. No markdown plan files. **Create cards as they crystallize during discussion, not as a batch at the end.** If a work item is defined enough to describe in conversation, it is defined enough to be a card. Don't wait for the user to ask for cards — the act of planning IS creating cards.

### Research before cards
Use `code_context` as the primary research tool. Always check blast radius (`op: "get blastradius"`) on files you expect to change — this is how you discover downstream work you'd otherwise miss. Use symbol search, call graphs, and text search (Glob/Grep/Read) to fill in the picture.

### Every card must be actionable

Card descriptions MUST include:

```
## What
<what to implement — full paths of files to create or modify, approach, context>

## Acceptance Criteria
- [ ] <observable outcome that proves the work is done>

## Tests
- [ ] <specific test to write or update, with file path>
- [ ] <test command to run and expected result>

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
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

### Specificity

Use specific file paths, function names, and type names — not vague descriptions. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" not "improve error handling."


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

## Autonomous Agent Mode

When operating as an autonomous agent (no Plan Mode UI), follow the `PLANNING_GUIDE.md` resource file bundled with this skill.

## Updating an Existing Plan

Update kanban cards directly — add new cards, update existing ones with `op: "update task"`, remove obsolete ones with `op: "delete task"`, and reorder dependencies as needed. The board is a living document.
