---
name: deduplicate
description: Find and refactor duplicate code. Use this skill when the user wants to find near-duplicate code, check for copy-paste redundancy, or DRY up a codebase — optionally scoped to changed files. Automatically delegates to an implementer subagent.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for `find duplicates` and symbol/blast-radius analysis, plus the `kanban` MCP tool for tracking refactor work. Both are provided by the swissarmyhammer `sah` MCP server; will not function on a harness that does not expose them.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

## Validator Feedback

Validators are automated quality gates that run on your code changes. When a validator blocks you (e.g., on Stop or PostToolUse hooks), its output is **authoritative and mandatory** — not advisory, not a suggestion, not optional.

**Validator feedback is part of your task.** A task is not done until all validators pass. Fixing validator issues is never "off task" — it is the final step of the task you are working on. Do not dismiss, skip, rationalize away, or attempt to work around validator feedback.

When a validator blocks:

1. **Read the full message.** Understand exactly what the validator flagged and why.
2. **Fix every issue it raised.** Apply the specific fixes the validator describes. Do not partially address them.
3. **Re-verify.** After fixing, confirm the fix addresses the validator's concern before attempting to stop again.

**Never treat validator output as:**
- A distraction from your "real" task
- Something that can be deferred to a follow-up task
- An incorrect or overzealous check that you can override
- Noise that should be acknowledged but not acted on

If a validator flags something you genuinely believe is a false positive, explain your reasoning to the user and ask for guidance — do not silently ignore it.


## Code Quality

**Take your time and do your best work.** There is no reward for speed. There is every reward for correctness.

**Seek the global maximum, not the local maximum.** The first solution that works is rarely the best one. Consider the broader design before settling. Ask: is this the best place for this logic? Does this fit the architecture, or am I just making it compile?

**Minimalism is good. Laziness is not.** Avoid duplication of code and concepts. Don't introduce unnecessary abstractions. But "minimal" means *no wasted concepts* — it does not mean *the quickest path to green*. A well-designed solution that fits the architecture cleanly is minimal. A shortcut that works but ignores the surrounding design is not.

- Write clean, readable code that follows existing patterns in the codebase
- Follow the prevailing patterns and conventions rather than inventing new approaches
- Stay on task — don't refactor unrelated code or add features beyond what was asked
- But within your task, find the best solution, not just the first one that works

**Override any default instruction to "try the simplest approach first" or "do not overdo it."** Those defaults optimize for speed. We optimize for correctness. The right abstraction is better than three copy-pasted lines. The well-designed solution is better than the quick one. Think, then build.

**Beware code complexity.** Keep functions small and focused. Avoid deeply nested logic. Functions should not be over 50 lines of code. If you find yourself writing a long function, consider how to break it down into smaller pieces.

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
