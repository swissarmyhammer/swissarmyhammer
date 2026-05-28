---
name: double-check
description: Double check your work by reviewing changes, asking clarifying questions, and verifying correctness before proceeding. Use when the user says "double check", "verify", "sanity check", or wants validation of recent work.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool x for symbol lookup and blast-radius checks used when verifying recent work.x
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Double Check

Review recent work for correctness, completeness, and alignment with intent.

## Process

1. **Gather context** — `git status`, `git get changes`, read the changed files, check active kanban tasks for the original requirements.

2. **Verify correctness** for each changed file:
   - Compiles/parses (run build or lint)
   - Matches intent (compare done vs asked)
   - No obvious bugs (off-by-one, missing error handling, typos, wrong variable names)
   - Tests pass for changed code
   - No loose ends: TODOs, commented-out code, debug prints, placeholders

3. **Clarify** — make a numbered list and ask one at a time, waiting for each answer. Specific and actionable, not vague.

4. **Report** — what's correct, what's broken, what's unclear.
