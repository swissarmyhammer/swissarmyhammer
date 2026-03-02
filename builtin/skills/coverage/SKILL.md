---
name: coverage
description: Analyze test coverage gaps on changed code. Scans branch changes, maps functions to tests structurally, and produces kanban cards for untested code. Use when the user says "coverage", "what's untested", "find coverage gaps", or wants to know what needs tests.
metadata:
  author: swissarmyhammer
  version: "2.0"
---

# Coverage

Identify test coverage gaps in changed code and produce a concrete work list of what needs tests.

## Process

### 1. Determine scope

- Default: files changed on the current branch vs `main`
- If the user specified files or a package, scope to that

### 2. Delegate to a tester subagent

Spawn a **tester** subagent with the specific goal of **coverage analysis, not test execution**. Tell it to:

- Scope to the changed files (use `git_changes` or `treesitter` as needed)
- For each changed function/method, determine whether a test exists that exercises it
- Create kanban cards for untested functions, tagged `["coverage-gap"]`
- Return a summary: what's covered, what's not, total gap count

This keeps verbose analysis (AST queries, file-by-file scanning) in the subagent's context instead of cluttering yours.

### 3. Relay results

When the subagent returns, present the summary to the user:

- Count of functions analyzed vs untested
- List of kanban cards created for coverage gaps
- Recommendation on where to start writing tests

## Guidelines

- The subagent does the analysis. You are the dispatcher — scope the work, delegate, relay results.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- If the user wants to write the missing tests, use the implement skill to pick up the kanban cards.
