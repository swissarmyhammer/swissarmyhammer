---
name: implement
description: Implementation workflow. Use this skill whenever you are implementing, coding, or building. Picks up one kanban card and does the work. Produces verbose output — automatically delegates to an implementer subagent.
metadata:
  author: "swissarmyhammer"
  version: "0.10.1"
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


# Implement

Pick up the next kanban card and get it done.

## Process

### 1. Get the next card

Use `kanban` with `op: "next task"` to find the next actionable card. If there are no remaining cards, tell the user the board is clear.

### 2. Move the card to doing

```json
{"op": "move task", "id": "<task-id>", "column": "doing"}
```

### 3. Read the card

```json
{"op": "get task", "id": "<task-id>"}
```

Get the full description and subtasks. Understand the task before writing code.

### 4. Read existing code

Read relevant code to understand patterns before writing. Never modify code you haven't read.

### 5. Implement the work

Do the work described in the card and its subtasks.

### 6. Complete the card

When all subtasks pass:

```json
{"op": "complete task", "id": "<task-id>"}
```

A card left in "doing" is not finished.

If you cannot complete the task, do NOT complete the card. Add a comment describing what happened and report back.

### 7. Stop for review

**Always stop after completing a card.** Present a summary of what was done and what tests pass. The user decides when to move to the next card — you do not auto-continue.

Only exception: if the card description explicitly says **auto-continue** or **chain to next**, proceed to the next card without stopping.

## Rules

- One card at a time. Don't try to do multiple cards in one pass.
- Do the work. No excuses, no "too complex". Find a way.
- Don't over-engineer — write the simplest code that works.
- Don't refactor unrelated code while implementing.
- Stay focused on the task you were given.
- ALL tests must pass before you report success. Zero failures, zero warnings.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- If you discover new work, add it as a new kanban card.
- If you get stuck, report what you tried and where you're blocked — don't silently give up.
