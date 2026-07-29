---
name: deduplicate
description: Find and refactor duplicate code. Use this skill when the user wants to find near-duplicate code, check for copy-paste redundancy, or DRY up a codebase. You can scope this to changed files.
agent: implementer
license: MIT OR Apache-2.0
compatibility: This skill requires the `code_context` MCP tool for `find duplicates` and for symbol and blast-radius analysis. It also requires the `kanban` MCP tool to track refactor work.
metadata:
  author: "swissarmyhammer"
  version: "{{version}}"
---


# Deduplicate

Find near-duplicates with tree-sitter semantic similarity, then refactor to remove redundancy.

## Process

1. **Scope:**
   - **Changed files** (default) — use `{"op": "get changes"}` from `git`
   - **Specific files** — files the user names
   - **Whole codebase** — a broad sweep

2. **Check the tree-sitter index**: `{"op": "get status"}`. The index must be ready before detection.

3. **Detect**: run `treesitter` `op: "find duplicates"` on the scoped files. For each cluster, note what is duplicated, the severity, and the refactor opportunity (extract function, module, or trait).

4. **Refactor** (only if requested, not for analysis alone):
   - Extract the shared logic
   - Replace the duplicates with calls
   - Run tests after each extraction
   - Follow TDD — if no test covers the code, write one

5. **Track on kanban** any cluster that needs a human decision:

   ```json
   {"op": "init board"}
   {"op": "add tag", "id": "duplicate", "name": "Duplicate Code", "color": "ff8800", "description": "Near-duplicate code needing refactoring"}
   {"op": "add task", "title": "<concise description>", "description": "<files and lines>\n\n<what's duplicated>\n\n<suggested refactoring>", "tags": ["duplicate"]}
   ```

6. **Summarize**: report the clusters by severity, the refactors done, the kanban tasks created, and a next-step recommendation.

## Rules

- Report only actionable duplication. Ignore test fixtures, generated code, trait-impl boilerplate, and single-line matches.
- Prefer the smallest extraction. Do not over-abstract.
- Note dependency implications when duplicates cross crates or packages.
- Kanban is the single source of truth — do not use TodoWrite or TaskCreate.
