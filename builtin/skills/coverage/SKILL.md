---
name: coverage
description: Analyze test coverage gaps on changed code. Scans branch changes, maps functions to tests structurally, and produces kanban cards for untested code. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests. Automatically delegates to a tester subagent.
context: fork
agent: tester
metadata:
  author: swissarmyhammer
  version: "3.0"
---

{% include "_partials/detected-projects" %}
{% include "_partials/coding-standards" %}

# Coverage

Identify test coverage gaps in changed code and produce a concrete work list of what needs tests.

**This is a coverage analysis task, not a test execution task.** Do not run or fix tests — analyze what's untested.

## Process

### 1. Determine scope

- Default: files changed on the current branch vs `main`
- If the user specified files or a package, scope to that
- If the user specified a commit range (e.g. "last 3 commits", "since abc123"), pass it as `range`

Use `git` with `op: "get changes"` to get the list of changed files:

```json
{"op": "get changes"}
```

With a range:

```json
{"op": "get changes", "range": "HEAD~3..HEAD"}
```

### 2. Analyze each changed file

For each changed file:

- Read the full file content
- Use `treesitter` with `op: "get status"` to check the index is ready
- Identify all public functions, methods, and types
- For each, determine whether a test exists that exercises it
- Look in the standard test locations for the project type

### 3. Track coverage gaps on the kanban board

Initialize the board and create a coverage-gap tag:

```json
{"op": "init board"}
```

```json
{"op": "add tag", "id": "coverage-gap", "name": "Coverage Gap", "color": "ff8800", "description": "Function or method lacking test coverage"}
```

Create a kanban card for each untested function:

```json
{"op": "add task", "title": "Add tests for <function_name>", "description": "<file:lines>\n\n<function signature>\n\n<what it does and what to test>", "tags": ["coverage-gap"]}
```

### 4. Summarize

Report:
- Count of functions analyzed vs untested
- List of kanban cards created for coverage gaps
- Recommendation on where to start writing tests

## Guidelines

- Do NOT run or fix tests — this is analysis only.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- Report only actionable gaps. Ignore: trivial getters/setters, trait impl boilerplate, generated code.
- If the user wants to write the missing tests, use the implement skill to pick up the kanban cards.
