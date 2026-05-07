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

A task without acceptance criteria and tests is not a valid task. Include enough context that someone reading only the task (not the spec) can implement it.

### Tests must be automated — never ask the user to verify

Every task's `Tests` section MUST specify **automated tests** (unit, integration, or end-to-end) that run in CI or via a test command. Do not write tasks that ask the user — or any human — to perform manual verification, smoke tests, click-throughs, or "try it out in the UI."

**Forbidden in task descriptions:**
- "Manually verify that…"
- "Smoke test by…"
- "User confirms…"
- "Open the app and check…"
- "Try it in the browser and make sure…"
- Any acceptance criterion whose only check is human observation.

**Also forbidden — soaks, burn-in, and other duration-gated runs:**
- "Soak test for N hours…", "let it run overnight…", "leave it running and check tomorrow…"
- "Burn-in for…", "stress test for…", "load test for N minutes…" as an acceptance check
- "Watch the dashboard / logs / metrics for a while to confirm…"
- "Run the app and use it for a while…"
- Any check whose pass condition is "elapsed wall-clock time without failure."

We are in the automated-testing business. Every check must be a deterministic, bounded automated test that runs in CI and produces pass/fail in seconds-to-minutes — not a job that asks a person or an agent to babysit a process. If the behavior genuinely needs duration to surface (memory leaks, retries, backoff, cache eviction), encode it as a fast deterministic test: fake the clock, inject the trigger, assert on observable state. Never spend wall-clock time watching.

**Required instead:**
- For backend/library code: unit tests and integration tests that exercise the real behavior.
- For APIs/services: integration tests against the real server (or a realistic harness).
- For UI: end-to-end tests (Playwright, Cypress, or equivalent) that drive the UI and assert on observable state.
- For bug fixes: a regression test that fails before the fix and passes after.
- For time-dependent behavior: inject a fake clock or scheduler and assert deterministically — do not gate on real elapsed time.

If the work is genuinely not testable automatically, that is a red flag — rescope the task or add a preceding task to make it testable. Our job is to do work for users, not to make work for them.

### Task sizing limits

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 generated or modified | > 500 lines |
| Files touched | 2–4 files | > 5 files |
| Subtasks | 3–5 per task | > 5 subtasks |
| Concerns | 1 per task | Multiple distinct concerns |

The subtask cap is the most important constraint. More than 5 subtasks means the task bundles multiple concerns — split along natural seams (different files, layers, or concerns) and link with `depends_on`. Two small tasks with a dependency beat one mega-task.

### Subtasks are checklist items in the description

Subtasks go in the task's `description` as GFM checklists (`- [ ]` items). There is no separate "add subtask" API.

### Specificity

Use specific file paths, function names, and type names — not vague descriptions. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" not "improve error handling."
