---
name: plan
description: Turn specifications into detailed implementation plans with actionable tasks. Use when the user has a spec, feature request, or design document that needs implementation steps.
metadata:
  author: swissarmyhammer
  version: "1.1"
---

# Plan

Create a comprehensive implementation plan from a specification, feature request, or design document.

The plan IS the kanban board. As you research and design, build the plan directly as kanban cards with subtasks — don't write a plan document and then "translate" it. The kanban board is the plan artifact.

## How to Execute

### Step 1: Initialize the board

Before exploring anything, set up the board: use `kanban` with `op: "init board"`, `name: "<project or feature name>"`

### Step 2: Research and build the plan incrementally

As you explore the codebase and understand what needs to happen, add cards to the kanban board immediately. Don't wait until you have a complete picture — each discovery becomes a card.

**When running inside Claude Code:**
1. **Enter plan mode** to analyze the specification and explore the codebase
2. **Research thoroughly** — read relevant files, understand the architecture, identify affected areas
3. **As you identify work items, create kanban cards** — each card is a piece of the plan
4. **Exit plan mode** with a summary for user approval — the cards are already on the board

**When running as an autonomous agent (llama-agent):**
Follow the planning process described in the `PLANNING_GUIDE.md` resource file bundled with this skill. As you work through each phase, add kanban cards for the work items you discover.

### Step 3: Structure each card

For each major work item, create a task: use `kanban` with `op: "add task"`, `title: "<what to implement>"`, `description: "<detailed context, affected files, approach>"`

Then add subtasks for individual steps: use `kanban` with `op: "add subtask"`, `task_id: "<task-id>"`, `title: "<specific step>"`

If tasks have ordering dependencies, set them: use `kanban` with `op: "update task"`, `id: "<task-id>"`, `depends_on: ["<blocker-task-id>"]`

## Important: Kanban Is the Single Source of Truth

Do NOT use any built-in task or todo tools (like TodoWrite or TaskCreate) to record the plan. The kanban board is how work is tracked across both Claude Code and llama-agent sessions. Every planned task belongs there as a card with subtasks.

## Guidelines

- Tasks should be sized so each one can be completed in a single session
- Subtasks should be concrete and verifiable — "add error handling to parse_config" not "improve code"
- Include enough context in task descriptions that someone (or the `do` skill) can execute without re-reading the spec
- Order tasks so foundational changes come first (data models, types) and dependent work follows
- Each task's subtasks should include running tests as the final step
- It's fine to rearrange, split, or merge cards as the plan evolves — the board is a living document
