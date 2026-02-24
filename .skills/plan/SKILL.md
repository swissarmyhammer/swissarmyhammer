---
name: plan
description: Plan Mode workflow. Use this skill whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban cards as the plan artifact.
metadata:
  author: "swissarmyhammer"
  version: "1.2"
---

# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work. The output of planning is a kanban board with cards and subtasks — NOT a markdown plan file.

## The Rule: Plans Are Kanban Cards

Do NOT write a plan as a markdown document. Do NOT use built-in task tools (TodoWrite, TaskCreate, TaskUpdate). Every planned work item becomes a kanban card with subtasks. The kanban board IS the plan.

When asking the user to review or approve the plan, present the kanban cards as the plan summary. Each card's title and description should clearly communicate the work to be done, and the subtasks should break it down into actionable steps. Expect the user to provide feedback on the cards themselves — they might ask you to add more detail to a card, split a card into two, or rearrange dependencies. This is how the plan evolves.

When you exit Plan Mode, there is no need for a plan markdown file -- we have the kanban cards. Instead, summarize the plan by listing the cards and their descriptions for the user to review.

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

### 4. Present the plan

When you believe the plan is ready, summarize the tasks you created on the board for the user. List each card with its title, a one-line description of what it covers, and any dependencies. This gives the user a clear picture of the planned work before they approve it.

When calling ExitPlanMode, the plan file should be this summary — not a separate document. The kanban cards contain the detail; the summary just enumerates what's on the board.

## How to Execute as an Autonomous Agent

Follow the planning process described in the `PLANNING_GUIDE.md` resource file bundled with this skill. As you work through each phase, add kanban cards for the work items you discover.

## Updating an Existing Plan

If the user asks to revise or extend the plan, update the kanban cards directly:
- Add new cards for new work
- Update existing cards with `op: "update task"`
- Remove obsolete cards with `op: "delete task"`
- Reorder dependencies as needed

## Guidelines

- Tasks should be sized so each one can be completed in a single session
- Subtasks should be concrete and verifiable — "add error handling to parse_config" not "improve code"
- Include enough context in task descriptions that someone (or the kanban skill) can execute without re-reading the spec
- Order tasks so foundational changes come first (data models, types) and dependent work follows
- Each task's subtasks should include running tests as the final step
- It's fine to rearrange, split, or merge cards as the plan evolves — the board is a living document
