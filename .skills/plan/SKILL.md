---
name: plan
description: Plan Mode workflow. Use this skill when the user says "/plan", "help me plan", "break this into tasks", "design the approach", or otherwise wants to plan work, and also whenever you are in Plan Mode. Drives all planning activity — research, task decomposition, and creating kanban tasks as the plan artifact.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for pre-plan research (symbol search, callgraph, blast-radius) and the `kanban` MCP tool for persisting the plan as kanban tasks.
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


# Plan

Use this skill whenever you enter Plan Mode or the user asks you to plan work.

## Goals

1. **Understand the work** — research the codebase deeply enough to know what needs to change and what will be affected.
2. **Produce a kanban board** — the plan artifact is kanban tasks with subtasks. Not a markdown document, not built-in task tools (TodoWrite, TaskCreate, TaskUpdate).
3. **Right-size the tasks** — each task is a single focused unit of work that can be independently implemented and verified.
4. **Collaborate with the user** — present tasks, discuss, iterate, and refine until the user is satisfied with the plan.
5. **Hand off cleanly** — when planning is complete, remind the user they can execute with `/finish` (autonomous) or `/implement` (one task at a time).

## Examples

### Example 1: a feature request turns into a decomposed kanban board

User says: "I want to add authentication to the app"

Actions:
1. Research with `code_context`: `{"op": "search symbol", "query": "user"}`, `{"op": "search symbol", "query": "session"}`, and `{"op": "get blastradius", "file_path": "src/server.rs", "max_hops": 3}` to find existing boundaries and callers that will need to participate.
2. As the design crystallizes in conversation, create tasks on the kanban board one at a time — not as a batch at the end:
   - `{"op": "add task", "title": "Design auth architecture", "description": "What: Decide JWT vs session, storage strategy. Acceptance Criteria: strategy documented in task comments; token format and expiry policy decided. Tests: no code tests — design task."}`
   - `{"op": "add task", "title": "Add User model and migration", "description": "What: Add users table and User struct in src/models/user.rs..."}`
   - `{"op": "add task", "title": "Implement POST /api/login", "description": "What: ...", "depends_on": ["<user-model-task-id>"]}`
3. Encode ordering with `depends_on` so foundational tasks (model, migration) come before integration (login endpoint).
4. Present the resulting board to the user, iterate on titles/descriptions/ordering.
5. When the user approves the plan, remind them: use `/finish` to drive the batch autonomously, or `/implement` for one task at a time. Do NOT call `ExitPlanMode` or start implementing.

Result: A kanban board with foundational → integration → test tasks, each independently implementable. The board IS the plan — no markdown plan file was created.

## Constraints

### Plans are kanban tasks — created as you go
Every planned work item becomes a kanban task. The kanban board IS the plan. No markdown plan files. **Create tasks as they crystallize during discussion, not as a batch at the end.** If a work item is defined enough to describe in conversation, it is defined enough to be a task. Don't wait for the user to ask for tasks — the act of planning IS creating tasks.

### Research before tasks
Use `code_context` as the primary research tool. Always check blast radius (`op: "get blastradius"`) on files you expect to change — this is how you discover downstream work you'd otherwise miss. Use symbol search, call graphs, and text search (Glob/Grep/Read) to fill in the picture.

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

**Required instead:**
- For backend/library code: unit tests and integration tests that exercise the real behavior.
- For APIs/services: integration tests against the real server (or a realistic harness).
- For UI: end-to-end tests (Playwright, Cypress, or equivalent) that drive the UI and assert on observable state.
- For bug fixes: a regression test that fails before the fix and passes after.

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


### Board naming
Name the board for the workspace/repository, not the specific feature being planned.

### User controls plan mode exit
Do NOT call ExitPlanMode yourself. The user decides when the plan is ready.

### No auto-implementation on exit
When the user exits plan mode or approves the plan, do NOT begin implementing. Instead, remind them:
- Use `/finish` to drive tasks all the way to `done` (implement → test → review) autonomously
- Use `/implement` to implement one task at a time

### Ordering
Foundational changes come first (data models, types, configuration), then core logic, then integration, then tests, then cleanup. Use `depends_on` to encode ordering constraints between tasks.

## Autonomous Agent Mode

When operating as an autonomous agent (no Plan Mode UI), follow the `references/PLANNING_GUIDE.md` resource file bundled with this skill.

## Updating an Existing Plan

Update kanban tasks directly — add new tasks, update existing ones with `op: "update task"`, remove obsolete ones with `op: "delete task"`, and reorder dependencies as needed. The board is a living document.
