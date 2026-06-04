---
name: task
profiles:
  - kanban
description: Create a single, well-researched kanban task. Use when the user wants to add a task, track an idea, or capture work without entering full plan mode.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for researching symbols and impact before writing the task, and the `kanban` MCP tool to persist the task on the board. Both are provided by the swissarmyhammer `sah` MCP server; will not function on a harness that does not expose them.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Task

Create one well-researched kanban task from an idea, request, or bug report.

$ARGUMENTS

## Constraints

- **One task per invocation.** Multiple items → pick the most important, suggest `/plan` for the rest.
- **Research before writing.** No guessing at paths, names, test locations.
- **Ask, don't assume.** Vague requests get the `question` tool.
- **Task quality is non-negotiable** — What + Acceptance Criteria + Tests.
- **Kanban only** — no TodoWrite/TaskCreate.

{% include "_partials/architecture-awareness" %}

{% include "_partials/task-standards" %}
