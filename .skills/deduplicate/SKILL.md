---
name: deduplicate
description: Find and refactor duplicate code. Use this skill when the user wants to find near-duplicate code, check for copy-paste redundancy, or DRY up a codebase — optionally scoped to changed files. Automatically delegates to an implementer subagent.
metadata:
  author: "swissarmyhammer"
  version: "3.0"
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

### Guidelines

- Write the test before the implementation
- Each test should verify one behavior
- Run tests frequently - after every small change
- Don't write new code without a failing test first
- If you find a bug, write a test that catches it before fixing
- All tests must pass, there is no such thing as a 'pre existing failure'. If a test is failing, assume you broke it -- because you did and just do not realize it.

### Test Structure

- **Arrange**: Set up the test conditions
- **Act**: Execute the code under test
- **Assert**: Verify the expected outcome

### When to Run Tests

- Before starting work (ensure clean baseline)
- After writing each new test (should fail)
- After writing implementation (should pass)
- Before committing (all tests must pass)


# Deduplicate

Find near-duplicate code using tree-sitter semantic similarity analysis, then refactor to eliminate redundancy.

## Process

### 1. Determine scope

- **Changed files** (default) — use `git` with `op: "get changes"` to get files modified on the current branch:

```json
{"op": "get changes"}
```

- **Specific files** — the user named files directly
- **Whole codebase** — the user asked for a broad sweep

### 2. Check the tree-sitter index

```json
{"op": "get status"}
```

Ensure the tree-sitter index is ready before running duplicate detection.

### 3. Run duplicate detection

Use `treesitter` with `op: "find duplicates"` on the scoped files. Analyze each duplicate cluster:

- What's duplicated and where
- Severity (how much code is repeated)
- Refactoring opportunity (extract function, shared module, trait, etc.)

### 4. Refactor duplicates

If the user wants refactoring (not just analysis):

- Extract shared logic into a function, module, or trait
- Replace each duplicate with a call to the shared code
- Run tests after each extraction to ensure nothing breaks
- Follow TDD — if no test covers the extracted code, write one

### 5. Track results on the kanban board

For duplicate clusters that need human decision before refactoring:

```json
{"op": "init board"}
```

```json
{"op": "add tag", "id": "duplicate", "name": "Duplicate Code", "color": "ff8800", "description": "Near-duplicate code needing refactoring"}
```

```json
{"op": "add task", "title": "<concise description>", "description": "<files and lines>\n\n<what's duplicated>\n\n<suggested refactoring>", "tags": ["duplicate"]}
```

### 6. Summarize

Report:
- Duplicate clusters found, grouped by severity
- What was refactored (if any)
- Kanban cards created for clusters needing decisions
- Recommendation on next steps

## Rules

- Report only actionable duplication. Ignore: test fixtures, generated code, trait impl boilerplate, and single-line matches.
- Prefer the smallest extraction that removes the duplication. Do not over-abstract.
- If duplicate code exists across different crates or packages, note the dependency implications.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
