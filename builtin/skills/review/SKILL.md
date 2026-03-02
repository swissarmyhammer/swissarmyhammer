---
name: review
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review. Delegates to a reviewer subagent to keep verbose review output out of the main context.
metadata:
  author: "swissarmyhammer"
  version: "2.0"
---

# Code Review

Perform a structured code review on the current changes.

## Process

### 1. Gather scope

Determine what is being reviewed:

- The current branch diff against `main` (default)
- A specific PR, branch, or commit range if the user specified one
- Specific files if the user pointed at them

### 2. Delegate to a reviewer subagent

Spawn a **reviewer** subagent to do the actual review. Pass it:

- The review scope (branch, PR, files)
- Any context about the purpose of the change (PR description, kanban card, user explanation)

The subagent performs structured, layered analysis, captures findings as kanban cards, and returns a summary with verdict.

This keeps verbose review output (file reads, line-by-line analysis, finding details) in the subagent's context instead of cluttering yours.

### 3. Relay results

When the subagent returns, present the summary to the user:

- Overall assessment and verdict (approve, request changes, comment only)
- Count of findings by severity
- List of kanban cards created
- Any nits reported inline

## Language-Specific Guidelines

The reviewer subagent has access to language-specific review guidelines bundled with the review skill:

| Language | Resource |
|----------|----------|
| Rust | `RUST_REVIEW.md` |
| Dart / Flutter | `DART_FLUTTER_REVIEW.md` |
| Python | `PYTHON_REVIEW.md` |
| JavaScript / TypeScript | `JS_TS_REVIEW.md` |

## Guidelines

- The subagent does the review. You are the dispatcher — scope the work, delegate, relay results.
- Do NOT use TodoWrite, TaskCreate, or any other task tracking — the kanban board is the single source of truth for findings.
- If the user wants to act on findings, use the implement skill to pick up the kanban cards.
