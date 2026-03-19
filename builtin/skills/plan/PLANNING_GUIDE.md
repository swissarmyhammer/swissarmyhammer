# Planning Guide for Autonomous Agents

This guide describes how to create a high-quality implementation plan when operating as an autonomous coding agent without a host IDE's planning mode.

## Goals

1. **Understand the work** — research the codebase deeply enough to know what needs to change and what will be affected.
2. **Produce a kanban board** — the plan artifact is kanban cards with subtasks. Not a markdown document.
3. **Right-size the cards** — each card is a single focused unit of work that can be independently implemented and verified.
4. **Collaborate with the user** — present cards, discuss, iterate, and refine until the user is satisfied.
5. **Hand off cleanly** — when planning is complete, remind the user they can execute with `/implement-loop` (autonomous) or `/implement` (one card at a time). Do NOT begin implementing.

## Constraints

### Research thoroughly before creating cards

Use `code_context` as the primary research tool:

- **Check index health** — `op: "get status"`. If incomplete, trigger with `op: "build status"`.
- **Detect projects** — `op: "detect projects"` for build commands and language guidelines.
- **Find symbols** — `op: "search symbol"` with domain keywords, `op: "get symbol"` for implementations, `op: "list symbols"` for file structure.
- **Map blast radius** — `op: "get blastradius"` on every file you expect to change. This is the most important research step — it reveals callers, downstream consumers, tests, and transitive dependencies. If blast radius is large, consider scoping the change more narrowly.
- **Trace call chains** — `op: "get callgraph"` with `direction: "inbound"` and `"outbound"` to understand execution flow.
- **Check recent history** — `git` with `op: "get changes"` on affected files.
- **Fall back to text search** — Glob, Grep, Read for string literals, config files, or patterns not in the index.

{% include "_partials/card-standards" %}

### Board naming
Name the board for the workspace/repository, not the specific feature.

### Ordering
Foundational changes first (data models, types, config), then core logic, then integration, then tests, then cleanup. Use `depends_on` for ordering constraints.

### Risks and open questions
Track unresolved questions as kanban cards so they stay visible.

### No auto-implementation
When the plan is approved, do NOT begin implementing. Remind the user:
- `/implement-loop` — implement all cards autonomously
- `/implement` — implement one card at a time

### Anti-patterns to avoid

- **Skipping blast radius** — leads to missed downstream work and surprise breakage.
- **Skipping exploration** — jumping to cards without reading code leads to wrong assumptions.
- **Unbounded searches** — scope to specific directories, not `**/*.rs`.
- **Vague tasks** — every card needs concrete, verifiable subtasks.
- **Mega-cards** — more than 5 subtasks or 5 files means split it.
- **Missing dependencies** — tasks that assume prior work but don't declare it.
- **Missing tests and acceptance criteria** — every card needs both.
