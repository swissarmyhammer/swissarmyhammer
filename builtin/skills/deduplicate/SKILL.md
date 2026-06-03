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


# Deduplicate

Find near-duplicates with tree-sitter semantic similarity, then refactor to remove redundancy.

## Process

1. **Scope:**
   - **Changed files** (default) — `{"op": "get changes"}` from `git`
   - **Specific files** — user-named
   - **Whole codebase** — broad sweep

2. **Check tree-sitter index**: `{"op": "get status"}` — must be ready before detection.

3. **Detect**: `treesitter` `op: "find duplicates"` on scoped files. For each cluster note: what's duplicated, severity, refactor opportunity (extract function, module, trait).

4. **Refactor** (if requested, not just analysis):
   - Extract shared logic
   - Replace duplicates with calls
   - Run tests after each extraction
   - Follow TDD — if no test covers it, write one

5. **Track on kanban** for clusters needing human decision:

   ```json
   {"op": "init board"}
   {"op": "add tag", "id": "duplicate", "name": "Duplicate Code", "color": "ff8800", "description": "Near-duplicate code needing refactoring"}
   {"op": "add task", "title": "<concise description>", "description": "<files and lines>\n\n<what's duplicated>\n\n<suggested refactoring>", "tags": ["duplicate"]}
   ```

6. **Summarize**: clusters by severity, refactors done, kanban tasks created, next-step recommendation.

## Rules

- Report only actionable duplication. Ignore test fixtures, generated code, trait-impl boilerplate, single-line matches.
- Prefer the smallest extraction; don't over-abstract.
- Note dependency implications when duplicates cross crates/packages.
- Kanban is the single source of truth — no TodoWrite/TaskCreate.
