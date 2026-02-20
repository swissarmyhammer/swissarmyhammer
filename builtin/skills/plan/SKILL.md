---
name: plan
description: Turn specifications into detailed implementation plans with actionable tasks. Use when the user has a spec, feature request, or design document that needs implementation steps.
allowed-tools: "*"
metadata:
  author: swissarmyhammer
  version: "1.0"
---

# Plan

Create a comprehensive implementation plan from a specification, feature request, or design document.

## How to Execute

### When running inside Claude Code

Use Claude Code's built-in planning mode:

1. **Enter plan mode** to analyze the specification and explore the codebase
2. **Research thoroughly** — read relevant files, understand the architecture, identify affected areas
3. **Design the approach** — determine what needs to change and in what order
4. **Exit plan mode** with your plan for user approval

Once the plan is approved, capture the results on the kanban board (see below).

### When running as an autonomous agent (llama-agent)

Follow the planning process described in the `PLANNING_GUIDE.md` resource file bundled with this skill. That guide covers the full planning workflow: understanding the request, exploring the codebase, assessing scope, and producing a structured plan.

## Capturing the Plan on the Kanban Board

After planning is complete, translate the plan into actionable kanban cards so it can be executed later with the `implement` or `do` skills:

1. Initialize the board if needed: use `kanban` with `op: "init board"`, `name: "<project or feature name>"`
2. For each major work item, create a task: use `kanban` with `op: "add task"`, `title: "<what to implement>"`, `description: "<detailed context, affected files, approach>"`
3. For each task, add subtasks for the individual steps: use `kanban` with `op: "add subtask"`, `task_id: "<task-id>"`, `title: "<specific step>"`
4. If tasks have ordering dependencies, set them: use `kanban` with `op: "update task"`, `id: "<task-id>"`, `depends_on: ["<blocker-task-id>"]`

## Guidelines

- Tasks should be sized so each one can be completed in a single session
- Subtasks should be concrete and verifiable — "add error handling to parse_config" not "improve code"
- Include enough context in task descriptions that someone (or the `do` skill) can execute without re-reading the spec
- Order tasks so foundational changes come first (data models, types) and dependent work follows
- Each task's subtasks should include running tests as the final step
