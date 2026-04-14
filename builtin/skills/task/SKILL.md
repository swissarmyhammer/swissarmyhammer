---
name: task
description: Create a single, well-researched kanban task. Use when the user wants to add a task, track an idea, or capture work without entering full plan mode.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}
{% include "_partials/task-standards" %}

# Task

Create a single, well-researched kanban task from an idea, request, or bug report.

{% if arguments %}
## User Request

> {{arguments}}
{% endif %}

## Process

### 1. Understand the idea

{% if arguments %}Start from the user request above.{% endif %} If anything is ambiguous or underspecified, use the `question` tool to ask clarifying questions before proceeding. A great task requires clear understanding — don't guess.

### 2. Research the codebase

Use `code_context` as the primary research tool:

- **Find symbols** — `op: "search symbol"` with domain keywords, `op: "get symbol"` for implementations
- **Map blast radius** — `op: "get blastradius"` on files you expect the work to touch. This reveals callers, downstream consumers, tests, and transitive dependencies.
- **Trace call chains** — `op: "get callgraph"` with `direction: "inbound"` and `"outbound"` to understand execution flow
- **Fall back to text search** — Glob, Grep, Read for string literals, config files, or patterns not in the index

Thorough research is always required. The tools you use may differ — a bug fix may focus on blast radius while a feature requires broader symbol exploration — but never skip research because something appears simple.

### 3. Create the task

Create the task on the kanban board using `kanban` with `op: "add task"`. The task must meet the task standards included above — What, Acceptance Criteria, and Tests sections are mandatory.

If the research reveals the work is too large for a single task (exceeds sizing limits), tell the user and suggest they use `/plan` instead.

### 4. Present the result

Show the user the task you created — title, description, and any tags applied.

## Constraints

- **One task per invocation.** If the user describes multiple pieces of work, create one task for the most important item and suggest `/plan` for the rest.
- **Research before writing.** Don't guess at file paths, function names, or test locations. Look them up.
- **Ask, don't assume.** If the user's request is vague or could be interpreted multiple ways, use the `question` tool to clarify before creating the task.
- **Task quality is non-negotiable.** Every task must have What, Acceptance Criteria, and Tests. A task without these is not valid.
- **Use the kanban board.** Do NOT use TodoWrite, TaskCreate, or any other task tracking. The kanban board is the single source of truth.
