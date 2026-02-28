---
name: plan
description: Plan Mode workflow. Use this skill whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban cards as the plan artifact.
metadata:
  author: swissarmyhammer
  version: "1.2"
---

# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work. The output of planning is a kanban board with cards — NOT a markdown plan file.

## The Rule: Plans Are Kanban Cards

Do NOT write a plan as a markdown document. Do NOT use built-in task tools (TodoWrite, TaskCreate, TaskUpdate). Every planned work item becomes a kanban card. The kanban board IS the plan.

When asking the user to review or approve the plan, present the kanban cards as the plan summary. Each card's title and description should clearly communicate the work to be done. Use markdown checklists (`- [ ]` items) in the description to break work into actionable steps. Expect the user to provide feedback on the cards themselves — they might ask you to add more detail to a card, split a card into two, or rearrange dependencies. This is how the plan evolves.

There is no need for a plan markdown file — the kanban cards ARE the plan.

## How to Execute in Claude Code Plan Mode

When you enter Plan Mode (via EnterPlanMode), follow these steps:

### 1. Ensure the kanban board exists

Use `kanban` with `op: "init board"`, `name: "<workspace name>"` — name it for the overall workspace or repository, not for the specific feature being planned. If the board already exists, this is a no-op; don't worry about it, just move on to research.

### 2. Research the codebase

Explore thoroughly — read relevant files, understand the architecture, identify affected areas. Use Glob, Grep, and Read tools to understand what exists.

### 3. Create kanban cards as you discover work

As you identify each work item, create a kanban card immediately. Don't wait until you have a complete picture — each discovery becomes a card.

For each work item: use `kanban` with `op: "add task"`, `title: "<what to implement>"`, `description: "<detailed context, affected files, approach>"`. Include a markdown checklist in the description to break the work into steps (e.g., `- [ ] step one\n- [ ] step two`).

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

## Guidelines

- Tasks should be sized so each one can be completed in a single session
- Checklist items in descriptions should be concrete and verifiable — "add error handling to parse_config" not "improve code"
- Include enough context in task descriptions that someone (or the kanban skill) can execute without re-reading the spec
- Order tasks so foundational changes come first (data models, types) and dependent work follows
- Each task's checklist should include running tests as the final step
- It's fine to rearrange, split, or merge cards as the plan evolves — the board is a living document
