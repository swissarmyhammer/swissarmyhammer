---
name: task
profiles:
  - kanban
description: Create one well-researched kanban task. Use this skill when the user wants to add a task, track an idea, or record work without a full planning session.
license: MIT OR Apache-2.0
compatibility: This skill needs the `code_context` MCP tool. It uses this tool to research symbols and impact before it writes the task. It also needs the `kanban` MCP tool, to save the task on the board. The swissarmyhammer `sah` MCP server provides both tools. The skill does not work on a harness that does not expose them.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Task

Create one well-researched kanban task from an idea, a request, or a bug report.

$ARGUMENTS

## Constraints

- **Create one task for each run.** If there are multiple items, pick the most important one. Suggest `/plan` for the rest.
- **Research before you write.** Do not guess paths, names, or test locations.
- **Ask; do not assume.** Use the `question` tool for a vague request.
- **Task quality is required.** Each task must state What, Acceptance Criteria, and Tests.
- **Use kanban only.** Do not use TodoWrite or TaskCreate.

{% include "_partials/architecture-awareness" %}

{% include "_partials/task-standards" %}

{% include "_partials/task-double-check" %}

{% include "_partials/short-ids" %}
