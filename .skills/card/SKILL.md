---
name: card
description: Create a single, well-researched kanban card. Use when the user wants to add a task, track an idea, or capture work without entering full plan mode.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/detected-projects" %}
{% include "_partials/coding-standards" %}
{% include "_partials/test-driven-development" %}
{% include "_partials/card-standards" %}

# Card

Create a single, well-researched kanban card from an idea, request, or bug report.

{% if arguments %}
## User Request

> {{arguments}}
{% endif %}

## Process

### 1. Understand the idea

{% if arguments %}Start from the user request above.{% endif %} If anything is ambiguous or underspecified, use the `question` tool to ask clarifying questions before proceeding. A great card requires clear understanding — don't guess.

### 2. Research the codebase

Use `code_context` as the primary research tool:

- **Find symbols** — `op: "search symbol"` with domain keywords, `op: "get symbol"` for implementations
- **Map blast radius** — `op: "get blastradius"` on files you expect the work to touch. This reveals callers, downstream consumers, tests, and transitive dependencies.
- **Trace call chains** — `op: "get callgraph"` with `direction: "inbound"` and `"outbound"` to understand execution flow
- **Fall back to text search** — Glob, Grep, Read for string literals, config files, or patterns not in the index

Research depth should match card complexity. A simple bug fix needs less exploration than a new feature that crosses module boundaries.

### 3. Create the card

Create the card on the kanban board using `kanban` with `op: "add task"`. The card must meet the card standards included above — What, Acceptance Criteria, and Tests sections are mandatory.

If the research reveals the work is too large for a single card (exceeds sizing limits), tell the user and suggest they use `/plan` instead.

### 4. Present the result

Show the user the card you created — title, description, and any tags applied.

## Constraints

- **One card per invocation.** If the user describes multiple pieces of work, create one card for the most important item and suggest `/plan` for the rest.
- **Research before writing.** Don't guess at file paths, function names, or test locations. Look them up.
- **Ask, don't assume.** If the user's request is vague or could be interpreted multiple ways, use the `question` tool to clarify before creating the card.
- **Card quality is non-negotiable.** Every card must have What, Acceptance Criteria, and Tests. A card without these is not valid.
- **Use the kanban board.** Do NOT use TodoWrite, TaskCreate, or any other task tracking. The kanban board is the single source of truth.
