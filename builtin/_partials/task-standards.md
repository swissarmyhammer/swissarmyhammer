---
title: Task Standards
description: Shared standards for kanban task quality — description template, sizing limits, subtask format, specificity
partial: true
---

### Every task must be actionable

Task descriptions MUST include:

```
## What
<what to implement — full paths of files to create or modify, approach, context>

## Acceptance Criteria
- [ ] <observable outcome that proves the work is done>

## Tests
- [ ] <specific automated test to write or update, with file path>
- [ ] <test command to run and expected result>

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
```

A task without acceptance criteria and tests is not valid. Include enough context that someone reading only the task can implement it.

### Tests must be automated — never ask the user to verify

Every `Tests` section MUST specify automated tests (unit, integration, or end-to-end) that run in CI or via a test command. Never ask a human to perform manual verification, smoke tests, click-throughs, or "try it in the UI."

**Forbidden:** "Manually verify…", "Smoke test by…", "User confirms…", "Open the app and check…", or any criterion whose only check is human observation.

**Required:**
- Backend/library: unit + integration tests against real behavior
- APIs/services: integration tests against the real server
- UI: end-to-end tests (Playwright, Cypress) driving the UI and asserting on observable state
- Bug fixes: a regression test that fails before the fix and passes after

If work is genuinely not testable automatically, rescope or add a preceding task to make it testable. Our job is to do work for users, not make work for them.

### Task sizing limits

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 | > 500 |
| Files touched | 2–4 | > 5 |
| Subtasks | 3–5 | > 5 |
| Concerns | 1 | Multiple |

The subtask cap is the strictest constraint. More than 5 means multiple concerns — split along natural seams and link with `depends_on`. Two small tasks with a dependency beat one mega-task.

### Subtasks are checklist items

Subtasks go in the `description` as GFM checklists (`- [ ]`). No separate "add subtask" API.

### Specificity

Use exact file paths, function names, and type names. "Add `Result` return type to `parse_config` and propagate errors to callers in `main.rs` and `cli.rs`" — not "improve error handling."
