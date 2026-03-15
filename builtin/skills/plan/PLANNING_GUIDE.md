# Planning Guide for Autonomous Agents

This guide describes how to create a high-quality implementation plan when operating
as an autonomous coding agent without a host IDE's planning mode.

## Phase 1: Understand the Request

Before exploring any code, make sure you understand what's being asked.

1. **Parse the goal** — identify whether this is a new feature, bug fix, refactor, or enhancement. This shapes everything that follows.
2. **Ask clarifying questions** — if the request is ambiguous, ask the user about requirements, constraints, and preferences before committing to an approach. Use the `question` tool with `op: "ask question"`.
3. **Identify acceptance criteria** — what does "done" look like? What should work when this is complete? What tests should pass?

## Phase 2: Explore the Codebase

Use `code_context` as your primary exploration tool. It provides indexed, structural code intelligence that is faster and more precise than raw text search. Follow a zoom-in funnel from broad to specific.

### Step 1: Check index health and project structure

- Run `code_context` with `op: "get status"` to confirm the index is ready. If indexing is incomplete, trigger a build with `op: "build status"`.
- Run `code_context` with `op: "detect projects"` to discover project types, build commands, and language-specific guidelines.
- Read project config files (`Cargo.toml`, `package.json`, `pyproject.toml`, etc.) for dependencies and build system details.

### Step 2: Find relevant symbols

- Use `code_context` with `op: "search symbol"` and domain keywords from the task to find relevant types, functions, and methods.
- Use `op: "get symbol"` to jump to definitions and read source text.
- Use `op: "list symbols"` to get a structural overview of key files before reading them in full.
- Use `op: "grep code"` for string literals, error messages, or patterns not captured by symbol indexing.

### Step 3: Map the blast radius

This is the most important exploration step. For each file or symbol you expect to change:

- Run `code_context` with `op: "get blastradius"`, `file_path: "<file>"`, `max_hops: 3` to discover everything that depends on it.
- The blast radius reveals callers, downstream consumers, tests, and transitive dependencies — work you'd otherwise miss.
- Use the results to identify files that need coordinated changes and tests that will be affected.
- If the blast radius is large (many files at hop 2-3), consider whether the change can be scoped more narrowly.

### Step 4: Trace call chains

- Use `code_context` with `op: "get callgraph"`, `direction: "inbound"` on key symbols to understand who calls them.
- Use `direction: "outbound"` to understand what they depend on.
- This reveals execution flow and helps identify the right boundaries for cards.

### Step 5: Find existing tests

- Search for test files covering the affected areas (check blast radius results — tests often appear at hop 1-2).
- Understand the testing patterns used (unit tests, integration tests, fixtures, mocks).
- Note the test runner and how tests are invoked.

### Step 6: Check recent history

- Use `git` with `op: "get changes"` to review recent changes to the affected files.
- This reveals change patterns, active development areas, and potential conflicts.

### When to fall back to raw search

Use Glob, Grep, and Read directly only for:
- Quick one-off string matches where you already know the exact text
- Config files, YAML, TOML, or other non-code files not in the index
- Exploring directory structure

## Phase 3: Assess Scope

Use the blast radius results from Phase 2 to classify the work concretely:

- **File count** — how many files appear in the blast radius at hop 1? (1-3 = small, 4-10 = medium, 10+ = large)
- **Cross-cutting concerns** — does the blast radius span multiple layers (API, business logic, database, UI)?
- **Dependency fan-out** — count unique files at hop 1-2 in the blast radius. High fan-out means more coordinated changes and more cards.
- **Test impact** — which test files appear in the blast radius? These need to be run and potentially updated.
- **Migration concerns** — does this affect data schemas, configurations, or external APIs?
- **Pattern consistency** — does the codebase have established patterns the change should follow?

If you haven't run blast radius yet for a file you plan to change, do it now. Every file in the plan should have its blast radius checked before cards are created.

Flag risks explicitly: breaking changes, data concerns, external API impacts, security implications.

## Phase 4: Build the Plan on the Kanban Board

The plan IS the kanban board. As you work through the phases above, create kanban cards incrementally — don't wait until you have a complete picture.

### Ensure the board exists

Use `kanban` with `op: "init board"`, `name: "<workspace name>"` — name it generically for the overall workspace or repository. If the board already exists, this is a no-op; just move on.

### Add cards as you discover work items

For each work item, create a card immediately: use `kanban` with `op: "add task"`, `title: "<imperative verb phrase>"`, `description: "<detailed context>"`.

Each card's description MUST include these sections:

```
## What
<what to implement — full paths of files to create or modify, approach, context>

## Acceptance Criteria
- [ ] <observable outcome that proves the work is done>
- [ ] <another criterion>

## Tests
- [ ] <specific test to write or update, with file path>
- [ ] <test command to run and expected result>
```

A card without acceptance criteria and tests is not a valid card. These sections ensure the definition of "done" is unambiguous and verifiable when the card is picked up for implementation.

Subtasks go in the card's `description` as GitHub Flavored Markdown checklists (`- [ ]` items). Include them when creating the card, or use `op: "update task"` to add them later. There is no separate "add subtask" API — subtasks live in the description.

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

## Phase 5: Present and Discuss

1. Present a summary of the kanban cards you created — list each card's title, a one-line description, and dependencies.
2. Stay conversational. Invite the user to discuss, iterate, and refine the cards. They may want to add detail, split or merge cards, rearrange dependencies, or ask clarifying questions.
3. Update the kanban cards based on feedback. Continue the discussion until the user is satisfied.
4. Let the user decide when the plan is ready — do not exit plan mode yourself. The board is ready for execution when the user says so.

## Card Sizing

Every card should be a single, focused unit of work. Use these limits:

| Dimension | Target | Split when |
|-----------|--------|------------|
| Lines of code | 200–500 generated or modified | > 500 lines |
| Files touched | 2–4 files | > 5 files |
| Subtasks | 3–5 per card | > 5 subtasks |
| Concerns | 1 per card | Multiple distinct concerns |

**The subtask cap is the most important constraint.** More than 5 subtasks means the card bundles multiple concerns — extract related subtasks into their own cards with `depends_on` links.

A subtask is a single code change: add a function, modify a struct, update a test file. If a subtask feels like a project, it should be its own card.

**How to split:** Look for seam lines — different files, different layers (data model vs. API vs. UI), different concerns (validation vs. persistence). Extract each group into its own card, link with dependencies, and ensure each card independently passes tests when complete.

Small cards (50–100 lines) are fine. Two small cards with a dependency beat one mega-card with a long checklist.

## What Makes a Good Plan

- **Specific file paths** for every change, not vague descriptions.
- **Code references** — mention specific functions, types, and patterns by name.
- **Right-sized cards** — each card targets 200–500 lines, 2–4 files, and 3–5 subtasks.
- **Acceptance criteria on every card** — observable outcomes that prove the work is done, not vague descriptions of intent.
- **Tests on every card** — specific test files to create or update, plus the test command and expected result. A card without tests is incomplete.
- **Sufficient context** — someone reading only the task description (not the spec) should understand what to do.

## Anti-Patterns to Avoid

- **Skipping blast radius** — creating cards without checking `get blastradius` on affected files leads to missed downstream work and surprise breakage.
- **Skipping exploration** — jumping to a plan without reading code leads to wrong assumptions.
- **Unbounded searches** — searching `**/*.rs` returns thousands of results. Scope searches to specific directories.
- **Vague tasks** — "improve error handling" is not actionable. "Add Result return type to parse_config and propagate errors to callers in main.rs and cli.rs" is.
- **Mega-cards** — if a card has more than 5 subtasks or touches more than 5 files, split it along natural seam lines.
- **Missing dependencies** — tasks that assume prior work was done but don't declare it.
- **No verification** — tasks without a way to confirm "done" are tasks that never finish.
- **Missing tests and acceptance criteria** — every card must have explicit `## Acceptance Criteria` and `## Tests` sections. Without these, the card is not actionable.
