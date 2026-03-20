---
name: card
description: Create a single, well-researched kanban card. Use when the user wants to add a task, track an idea, or capture work without entering full plan mode.
metadata:
  author: "swissarmyhammer"
  version: "0.9.2"
---

## Project Detection

To discover project types, build commands, and language-specific guidelines for this workspace, call the code_context tool:

```json
{"op": "detect projects"}
```

This will scan the directory tree and return:
- All detected project types (Rust, Node.js, Python, Go, Java, C#, CMake, Makefile, Flutter, PHP)
- Project locations as relative paths
- Workspace/monorepo membership
- Language-specific guidelines for testing, building, formatting, and linting

**Call this early in your session** to understand the project structure before making changes. The guidelines returned are authoritative — follow them for test commands, build commands, and formatting.

** Fix the root cause, not the symptoms **

## Code Quality

- Write clean, readable code that follows existing patterns in the codebase
- Prefer simple, obvious solutions over clever ones
- Make minimal changes to achieve the goal - avoid unnecessary refactoring
- Don't add features, abstractions, or "improvements" beyond what was asked

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

## Test Driven Development

Write tests first, then implementation. This ensures code is testable and requirements are clear.

### TDD Cycle

1. **Red**: Write a failing test that defines what you want
2. **Green**: Write the minimum code to make the test pass
3. **Refactor**: Clean up while keeping tests green

### When to Run Tests

- Before starting work (ensure clean baseline)
- After writing each new test (should fail)
- After writing implementation (should pass)
- Before committing (all tests must pass)

### Every card must be actionable

Card descriptions MUST include:

```
## What
<what to implement — full paths of files to create or modify, approach, context>

## Acceptance Criteria
- [ ] <observable outcome that proves the work is done>

## Tests
- [ ] <specific test to write or update, with file path>
- [ ] <test command to run and expected result>
```

A card without acceptance criteria and tests is not a valid card. Include enough context that someone reading only the card (not the spec) can implement it.

### Card sizing limits

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 generated or modified | > 500 lines |
| Files touched | 2–4 files | > 5 files |
| Subtasks | 3–5 per card | > 5 subtasks |
| Concerns | 1 per card | Multiple distinct concerns |

The subtask cap is the most important constraint. More than 5 subtasks means the card bundles multiple concerns — split along natural seams (different files, layers, or concerns) and link with `depends_on`. Two small cards with a dependency beat one mega-card.

### Subtasks are checklist items in the description

Subtasks go in the card's `description` as GFM checklists (`- [ ]` items). There is no separate "add subtask" API.

### Specificity

Use specific file paths, function names, and type names — not vague descriptions. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" not "improve error handling."


# Card

Create a single, well-researched kanban card from an idea, request, or bug report.



## Process

### 1. Understand the idea

 If anything is ambiguous or underspecified, use the `question` tool to ask clarifying questions before proceeding. A great card requires clear understanding — don't guess.

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
