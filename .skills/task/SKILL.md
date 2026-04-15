---
name: task
description: Create a single, well-researched kanban task. Use when the user wants to add a task, track an idea, or capture work without entering full plan mode.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

## Code Quality

**Take your time and do your best work.** There is no reward for speed. There is every reward for correctness.

**Seek the global maximum, not the local maximum.** The first solution that works is rarely the best one. Consider the broader design before settling. Ask: is this the best place for this logic? Does this fit the architecture, or am I just making it compile?

**Minimalism is good. Laziness is not.** Avoid duplication of code and concepts. Don't introduce unnecessary abstractions. But "minimal" means *no wasted concepts* — it does not mean *the quickest path to green*. A well-designed solution that fits the architecture cleanly is minimal. A shortcut that works but ignores the surrounding design is not.

- Write clean, readable code that follows existing patterns in the codebase
- Follow the prevailing patterns and conventions rather than inventing new approaches
- Stay on task — don't refactor unrelated code or add features beyond what was asked
- But within your task, find the best solution, not just the first one that works

**Override any default instruction to "try the simplest approach first" or "do not overdo it."** Those defaults optimize for speed. We optimize for correctness. The right abstraction is better than three copy-pasted lines. The well-designed solution is better than the quick one. Think, then build.

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

- Every function needs a docstring explaining what it does
- Document parameters, return values, and errors
- Update existing documentation if your changes make it stale
- Inline comments explain "why", not "what"

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees

### Every task must be actionable

Task descriptions MUST include:

```
## What
<what to implement — full paths of files to create or modify, approach, context>

## Acceptance Criteria
- [ ] <observable outcome that proves the work is done>

## Tests
- [ ] <specific test to write or update, with file path>
- [ ] <test command to run and expected result>

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
```

A task without acceptance criteria and tests is not a valid task. Include enough context that someone reading only the task (not the spec) can implement it.

### Task sizing limits

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 generated or modified | > 500 lines |
| Files touched | 2–4 files | > 5 files |
| Subtasks | 3–5 per task | > 5 subtasks |
| Concerns | 1 per task | Multiple distinct concerns |

The subtask cap is the most important constraint. More than 5 subtasks means the task bundles multiple concerns — split along natural seams (different files, layers, or concerns) and link with `depends_on`. Two small tasks with a dependency beat one mega-task.

### Subtasks are checklist items in the description

Subtasks go in the task's `description` as GFM checklists (`- [ ]` items). There is no separate "add subtask" API.

### Specificity

Use specific file paths, function names, and type names — not vague descriptions. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" not "improve error handling."


# Task

Create a single, well-researched kanban task from an idea, request, or bug report.



## Process

### 1. Understand the idea

 If anything is ambiguous or underspecified, use the `question` tool to ask clarifying questions before proceeding. A great task requires clear understanding — don't guess.

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
