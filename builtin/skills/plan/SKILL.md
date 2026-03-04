---
name: plan
description: Plan Mode workflow. Use this skill whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban cards as the plan artifact.
metadata:
  author: swissarmyhammer
  version: "1.2"
---

# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work. The output of planning is a kanban board with cards and subtasks — NOT a markdown plan file.

## The Rule: Plans Are Kanban Cards

Do NOT write a plan as a markdown document. Do NOT use built-in task tools (TodoWrite, TaskCreate, TaskUpdate). Every planned work item becomes a kanban card with subtasks. The kanban board IS the plan.

When asking the user to review or approve the plan, present the kanban cards as the plan summary. Each card's title and description should clearly communicate the work to be done, and the subtasks should break it down into actionable steps. Expect the user to provide feedback on the cards themselves — they might ask you to add more detail to a card, split a card into two, or rearrange dependencies. This is how the plan evolves.

There is no need for a plan markdown file — the kanban cards ARE the plan.

## How to Execute in Claude Code Plan Mode

When you enter Plan Mode (via EnterPlanMode), follow these steps:

### 1. Ensure the kanban board exists

Use `kanban` with `op: "init board"`, `name: "<workspace name>"` — name it for the overall workspace or repository, not for the specific feature being planned. If the board already exists, this is a no-op; don't worry about it, just move on to research.

### 2. Research the codebase

Explore thoroughly — read relevant files, understand the architecture, identify affected areas. Use Glob, Grep, and Read tools to understand what exists.

### 3. Create kanban cards as you discover work

As you identify each work item, create a kanban card immediately. Don't wait until you have a complete picture — each discovery becomes a card.

For each work item: use `kanban` with `op: "add task"`, `title: "<what to implement>"`, `description: "<detailed context, affected files, approach>"`

Then add subtasks: use `kanban` with `op: "add subtask"`, `task_id: "<task-id>"`, `title: "<specific step>"`

Set dependencies between cards: use `kanban` with `op: "update task"`, `id: "<task-id>"`, `depends_on: ["<blocker-task-id>"]`

### 4. Present the plan and discuss

When you believe the initial plan is complete, present a summary to the user. List each card with its title, a one-line description of what it covers, and any dependencies. This gives the user a clear picture of the planned work.

**Stay conversational.** After presenting the summary, invite the user to discuss, ask questions, and iterate. They might want to:
- Add more detail to a card or split it into multiple cards
- Merge cards that feel too granular
- Rearrange dependencies or reorder work
- Add cards you missed or remove ones that aren't needed
- Ask clarifying questions about your approach

Update the kanban cards based on their feedback. The planning conversation continues until the user is satisfied — do NOT call ExitPlanMode yourself. Let the user decide when the plan is ready. If they ask to proceed, approve the plan, or start implementation, then you can exit plan mode.

## How to Execute as an Autonomous Agent

Follow the planning process described in the `PLANNING_GUIDE.md` resource file bundled with this skill. As you work through each phase, add kanban cards for the work items you discover.

## Updating an Existing Plan

If the user asks to revise or extend the plan, update the kanban cards directly:
- Add new cards for new work
- Update existing cards with `op: "update task"`
- Remove obsolete cards with `op: "delete task"`
- Reorder dependencies as needed

## Card Sizing

A card should represent a single, focused unit of work. Use these limits to keep cards right-sized:

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 generated or modified | > 500 lines |
| Files touched | 2–4 files | > 5 files |
| Subtasks | 3–5 per card | > 5 subtasks |
| Concerns | 1 per card | Multiple distinct concerns |

### Why these limits matter

- **Subtask cap is the most important lever.** If a card needs more than 5 subtasks, it is bundling multiple concerns. Extract groups of related subtasks into their own cards with dependencies between them.
- **A subtask is a single code change** — add a function, modify a struct, update a test file. If a subtask itself feels like a project, it should be its own card.
- **Small cards are fine.** Some cards are legitimately 50–100 lines (add a field, wire a dependency). The floor is not a target — only the ceiling matters.
- **When in doubt, split.** Two small cards with a dependency are always better than one mega-card with a long checklist.

### How to split an oversized card

1. Look for natural seam lines: different files, different layers (data model vs. API vs. UI), different concerns (validation vs. persistence vs. rendering).
2. Extract each group into its own card with a clear title and description.
3. Add `depends_on` links so execution order is preserved.
4. Each resulting card should independently pass tests when complete.

## Guidelines

- Subtasks should be concrete and verifiable — "add error handling to parse_config" not "improve code"
- Include enough context in task descriptions that someone (or the kanban skill) can execute without re-reading the spec
- Order tasks so foundational changes come first (data models, types) and dependent work follows
- Each task's subtasks should include running tests as the final step
- It's fine to rearrange, split, or merge cards as the plan evolves — the board is a living document
