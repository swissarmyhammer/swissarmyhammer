# Planning Guide for Autonomous Agents

This guide describes how to create a high-quality implementation plan when operating
as an autonomous coding agent without a host IDE's planning mode.

## Phase 1: Understand the Request

Before exploring any code, make sure you understand what's being asked.

1. **Parse the goal** — identify whether this is a new feature, bug fix, refactor, or enhancement. This shapes everything that follows.
2. **Ask clarifying questions** — if the request is ambiguous, ask the user about requirements, constraints, and preferences before committing to an approach. Use the `question_ask` tool.
3. **Identify acceptance criteria** — what does "done" look like? What should work when this is complete? What tests should pass?

## Phase 2: Explore the Codebase

Follow a zoom-in funnel from broad to specific. Do NOT chain `cd`, `ls`, `cat` commands — use purpose-built search tools.

### Step 1: Project structure and configuration

- Read project config files (`Cargo.toml`, `package.json`, `pyproject.toml`, etc.) to understand the tech stack, dependencies, and build system.
- Use `files_glob` to list the top-level directory structure and understand project organization.
- Check for project conventions in `CLAUDE.md`, `AGENTS.md`, or `.swissarmyhammer/` directories.

### Step 2: Keyword search for relevant areas

- Use `files_grep` with keywords from the task description to locate relevant files across the codebase.
- Search for type names, function names, error messages, or domain terms mentioned in the request.
- Use `treesitter_search` to find definitions of types, functions, and structs by name.

### Step 3: Read relevant files

- Read the most relevant files found in step 2. Follow imports and references to understand dependency chains.
- Use `treesitter_query` to extract structure (function signatures, type definitions) without reading entire files.
- Pay attention to patterns: how is error handling done? What naming conventions are used? How are tests structured?

### Step 4: Find existing tests

- Search for test files covering the affected areas.
- Understand the testing patterns used (unit tests, integration tests, fixtures, mocks).
- Note the test runner and how tests are invoked.

### Step 5: Check recent history

- Use `git_changes` to review recent changes to the affected files.
- This reveals change patterns, active development areas, and potential conflicts.

## Phase 3: Assess Scope

Based on your exploration, classify the work:

- **File count** — how many files need modification? (1-3 = small, 4-10 = medium, 10+ = large)
- **Cross-cutting concerns** — does this touch multiple layers (API, business logic, database, UI)?
- **Dependency fan-out** — how many other files import or reference the files being changed?
- **Test impact** — are there existing tests? Will new tests be needed? Could this break existing tests?
- **Migration concerns** — does this affect data schemas, configurations, or external APIs?
- **Pattern consistency** — does the codebase have established patterns the change should follow?

Flag risks explicitly: breaking changes, data concerns, external API impacts, security implications.

## Phase 4: Build the Plan on the Kanban Board

The plan IS the kanban board. As you work through the phases above, create kanban cards incrementally — don't wait until you have a complete picture.

### Initialize the board first

Use `kanban` with `op: "init board"`, `name: "<project or feature name>"`.

### Add cards as you discover work items

For each work item, create a card immediately: use `kanban` with `op: "add task"`, `title: "<imperative verb phrase>"`, `description: "<detailed context>"`.

Each card's description should include:
- What specifically to do, with enough context for autonomous execution
- Full paths of files to create or modify
- How to verify the task is done (test command, expected behavior)

Then add subtasks for individual steps: use `kanban` with `op: "add subtask"`, `task_id: "<task-id>"`, `title: "<specific step>"`.

### Set dependencies between cards

If tasks have ordering constraints, set them: use `kanban` with `op: "update task"`, `id: "<task-id>"`, `depends_on: ["<blocker-task-id>"]`.

### Task ordering

Group tasks by phase:
1. **Infrastructure** — data models, types, configuration, dependencies
2. **Core implementation** — the main logic changes
3. **Integration** — connecting components, API changes, UI updates
4. **Tests** — new tests, updating existing tests
5. **Cleanup** — documentation, removing dead code, formatting

### Risks and Open Questions

If there are unresolved questions, add a kanban card for each one so they are tracked and visible.

## Phase 5: Present and Gate

1. Present the plan to the user for review using `question_ask`. Summarize the kanban cards you created.
2. Accept edits, rejections, or approvals. Update the board accordingly.
3. The board is ready for execution — no separate "capture" step needed.

## What Makes a Good Plan

- **Specific file paths** for every change, not vague descriptions.
- **Code references** — mention specific functions, types, and patterns by name.
- **Bounded tasks** — each one completable in a single focused session.
- **Independent verification** — each task has its own success criterion.
- **Sufficient context** — someone reading only the task description (not the spec) should understand what to do.
- **Test-inclusive** — every task ends with running tests.

## Anti-Patterns to Avoid

- **Skipping exploration** — jumping to a plan without reading code leads to wrong assumptions.
- **Unbounded searches** — searching `**/*.rs` returns thousands of results. Scope searches to specific directories.
- **Vague tasks** — "improve error handling" is not actionable. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" is.
- **Monolithic tasks** — if a task touches more than 3-4 files, it should probably be split.
- **Missing dependencies** — tasks that assume prior work was done but don't declare it.
- **No verification** — tasks without a way to confirm "done" are tasks that never finish.
