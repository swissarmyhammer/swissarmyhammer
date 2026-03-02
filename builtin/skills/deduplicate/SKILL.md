---
name: deduplicate
description: Find and refactor duplicate code. Use this skill when the user wants to find near-duplicate code, check for copy-paste redundancy, or DRY up a codebase — optionally scoped to changed files. Delegates analysis to a subagent to keep verbose output out of the main context.
metadata:
  author: "swissarmyhammer"
  version: "2.0"
---

# Deduplicate

Find near-duplicate code using tree-sitter semantic similarity analysis, then refactor to eliminate redundancy.

## Process

### 1. Determine scope

- **Changed files** (default) — use `git_changes` to get files modified on the current branch
- **Specific files** — the user named files directly
- **Whole codebase** — the user asked for a broad sweep

### 2. Delegate to a subagent

Spawn an **implementer** subagent to do the analysis and refactoring. Pass it the scope and tell it to:

- Check the tree-sitter index is ready (`treesitter` with `op: "get status"`)
- Run duplicate detection on the scoped files (`treesitter` with `op: "find duplicates"`)
- Analyze each duplicate cluster: what's duplicated, where, severity, refactoring opportunity
- If the user wants refactoring: extract shared logic, replace duplicates, run tests after each extraction
- Create kanban cards for duplicate clusters that need human decision before refactoring
- Return a summary: clusters found, severity, what was refactored, what needs decisions

This keeps verbose analysis (AST queries, file-by-file scanning, refactoring iterations) in the subagent's context instead of cluttering yours.

### 3. Relay results

When the subagent returns, present the summary to the user:

- Duplicate clusters found, grouped by severity
- What was refactored (if any)
- Kanban cards created for clusters needing decisions
- Recommendation on next steps

## Guidelines

- The subagent does the work. You are the dispatcher — scope the work, delegate, relay results.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
- Report only actionable duplication. Ignore: test fixtures, generated code, trait impl boilerplate, and single-line matches.
- Prefer the smallest extraction that removes the duplication. Do not over-abstract.
- If duplicate code exists across different crates or packages, note the dependency implications.
