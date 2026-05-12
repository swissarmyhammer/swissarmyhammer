---
name: deduplicate
description: Find and refactor duplicate code. Use this skill when the user wants to find near-duplicate code, check for copy-paste redundancy, or DRY up a codebase — optionally scoped to changed files. Automatically delegates to an implementer subagent.
context: fork
agent: implementer
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for `find duplicates` and symbol/blast-radius analysis, plus the `kanban` MCP tool for tracking refactor work.
metadata:
  author: "swissarmyhammer"
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}

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
- Kanban tasks created for clusters needing decisions
- Recommendation on next steps

## Rules

- Report only actionable duplication. Ignore: test fixtures, generated code, trait impl boilerplate, and single-line matches.
- Prefer the smallest extraction that removes the duplication. Do not over-abstract.
- If duplicate code exists across different crates or packages, note the dependency implications.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth.
