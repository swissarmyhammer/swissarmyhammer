---
name: review
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review. Reviews produce verbose output — automatically delegates to a reviewer subagent.
context: fork
agent: reviewer
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool (for symbol lookup, callgraph, and blast-radius during review) and the `kanban` MCP tool (to drive tasks through the review column and capture follow-up findings). 
metadata:
  author: "swissarmyhammer"
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}
{% include "_partials/review-column" %}

# Code Review

Perform a structured code review. Findings land as a GFM checklist on a kanban task — so they stay attached to the work they describe rather than piling up as new tasks that clog the board.

## Process

### 1. Ensure the Review Column

Before anything else, ensure the `review` column exists by following the procedure in the **Ensure the Review Column Exists** partial above. It is idempotent — run it every time.

### 2. Determine the Mode

The review skill operates in one of two modes, chosen by how it was invoked:

| Invocation | Mode |
|------------|------|
| `/review <task-id>` | **task-mode** on that specific task |
| Bare `/review` with one or more tasks in the `review` column | **task-mode** on the oldest task in the `review` column |
| Bare `/review` with the `review` column empty | **range-mode** on the current branch's changes |
| `/review HEAD~4..HEAD`, `/review since abc123`, `/review feature-branch`, etc. | **range-mode** on that range/branch |

To check the `review` column when bare `/review` is invoked:

```json
{"op": "list tasks", "column": "review"}
```

If there are tasks in that column, pick the **oldest** (lowest ordinal / earliest created) and enter task-mode with its id.

### 3. Get the Changes

Use `git` with `op: "get changes"` to scope the diff.

**Task-mode**: start by reading the task:

```json
{"op": "get task", "id": "<task-id>"}
```

Use any range hint in the task's description (a commit range, a branch name, or a PR reference) to scope the diff. If the task gives no explicit hint, call `{"op": "get changes"}` and let it auto-detect.

**Range-mode** — parse the user's natural language and map it:

| User says | `get changes` call |
|-----------|-------------------|
| `/review` (nothing else, `review` column empty) | `{"op": "get changes"}` — auto-detects branch or defaults to last commit on main |
| `/review the last 4 commits` | `{"op": "get changes", "range": "HEAD~4..HEAD"}` |
| `/review since abc123` | `{"op": "get changes", "range": "abc123..HEAD"}` |
| `/review abc123..def456` | `{"op": "get changes", "range": "abc123..def456"}` |
| `/review feature-branch` | `{"op": "get changes", "branch": "feature-branch"}` |

Read the full content of every changed file — diffs alone lack context. Understand the **purpose** of the change before reviewing (PR description, commit messages, kanban task body).

When a `range` was used (explicit or auto-defaulted), use `get diff` with `file@<start-ref>` / `file@<end-ref>` syntax for semantic diffs:

```json
{"op": "get diff", "left": "src/main.rs@HEAD~4", "right": "src/main.rs"}
```

### 4. Layered Examination

Review in progressive layers. Do not skip layers — each catches different classes of problems.

**Layer 1: Design and Architecture** — Does the change fit the system? Appropriate abstractions? Over-engineering? Does it belong in this codebase or in a library?

**Layer 2: Functionality and Correctness** — Does the code do what the author intended? Is that good for users? Edge cases: empty inputs, nulls, boundary values, error conditions. Off-by-one errors, incorrect boolean logic, missing early returns. Concurrency: race conditions, deadlocks, shared mutable state.

**Layer 3: Tests** — Tests for new/changed behavior? Do they verify behavior, not implementation? Would they fail if the code were broken? Edge cases covered? Mocks only at system boundaries?

**Layer 4: Security** — Input validated and sanitized? Injection risks (SQL, command, XSS, template)? Secrets handled safely? Auth checks in place? Error messages safe?

**Layer 5: Naming, Clarity, Simplicity** — Names descriptive without being verbose? Code understandable without explanation? Comments explain "why", not "what"? Stale comments or TODOs? **Doc comments** must state the current contract, not history — flag any `///` that uses *previously / used to / legacy / now / was added for*, references a task/PR/issue id, restates the signature, walks internal dispatch branches, lists callers, or runs longer than the budget (1 sentence default for fns, 3 hard ceiling). See the `doc-comments` skill for the full rule set and rewrite process.

**Layer 6: Performance** (when relevant) — O(n^2) or worse on large data? Unnecessary allocations in hot paths? N+1 queries? Resource cleanup in all paths?

### 5. Review Every Line

Look at every line of changed code. If code is hard to understand, that is itself a finding.

### 6. Apply Language-Specific Guidelines

Read the matching resource file bundled with this skill:

| Language | File |
|----------|------|
| Rust | [RUST_REVIEW.md](./references/RUST_REVIEW.md) |
| Dart / Flutter | [DART_FLUTTER_REVIEW.md](./references/DART_FLUTTER_REVIEW.md) |
| Python | [PYTHON_REVIEW.md](./references/PYTHON_REVIEW.md) |
| JavaScript / TypeScript | [JS_TS_REVIEW.md](./references/JS_TS_REVIEW.md) |

If the project uses multiple languages, apply all relevant sections. Language-specific findings follow the same severity levels.

### 7. Architecture Review

If an `ARCHITECTURE.md` file exists at the project root, read it and add an architecture alignment layer to your review:

- **Does the change follow the documented architecture?** Check that new code lands in the correct module/layer/component. Flag changes that bypass documented boundaries (e.g., a handler directly calling the database when the architecture specifies a service layer).
- **Are new components placed correctly?** If the change introduces new files, modules, or crates, verify they fit the structure described in `ARCHITECTURE.md`.
- **Does the change require an architecture update?** If the change intentionally diverges from or extends the documented architecture (new module, new dependency direction, new service), include a finding recommending that `ARCHITECTURE.md` be updated to reflect the new state.

Architecture findings use the same severity levels — a boundary violation is a **warning**, an undocumented structural addition is a **nit** unless it contradicts an explicit constraint (then **blocker**).

If no `ARCHITECTURE.md` exists, skip this step.

### 8. Format Findings as a Dated GFM Checklist

Organize findings by severity within a single dated section. Use the current local date and time for the heading:

```markdown
## Review Findings (2026-04-11 13:08)

### Blockers
- [ ] `path/to/file.rs:42` — What's wrong. Why it matters. Suggested fix.

### Warnings
- [ ] `path/to/file.rs:10` — What's wrong and suggested fix.

### Nits
- [ ] `path/to/file.rs:88` — Minor issue.
```

Severity guide:

| Severity | Meaning |
|----------|---------|
| **blocker** | Correctness bug, security vulnerability, data loss risk |
| **warning** | Design problem, missing test, performance concern |
| **nit** | Style preference, minor improvement |

Each finding must be specific and actionable: **where** (file:line), **what**, **why** (skip for nits), and **suggestion** when non-obvious.

Omit empty severity subsections — if there are no blockers, don't include a `### Blockers` heading at all.

**One concern per checklist item.** Don't bundle unrelated issues into a single bullet. If three components each have the same problem, that's three items, not one item listing three components.

### 9. Apply the Findings

The review skill never creates one kanban task per finding. Instead, findings become checklist items on a host task — either the task being reviewed (task-mode) or a single tracking task for the range (range-mode).

#### Task-mode

1. Re-read the target task (you already have it from step 3):

   ```json
   {"op": "get task", "id": "<task-id>"}
   ```

2. If the task is not currently in the `review` column, move it there first so the board reflects its state:

   ```json
   {"op": "move task", "id": "<task-id>", "column": "review"}
   ```

   This handles the case where someone runs `/review <task-id>` manually on a task still sitting in `todo` or `doing`. Tasks that came in from `implement` are already in `review` and this is a no-op.

3. Parse the `description` for any prior `## Review Findings (...)` sections and note whether every `- [ ]` in those prior sections has been flipped to `- [x]`.

4. Decide the outcome:

   - **Fresh review produced zero findings AND every prior checklist item is checked** → move the task past review to the terminal column:

     ```json
     {"op": "move task", "id": "<task-id>", "column": "done"}
     ```

     Do not otherwise modify the description — leave the history of prior review sections intact.

   - **Fresh review produced findings, OR any prior checklist item is still unchecked** → append the new dated `## Review Findings (YYYY-MM-DD HH:MM)` section to the existing description and write it back:

     ```json
     {"op": "update task", "id": "<task-id>", "description": "<existing description + blank line + new section>"}
     ```

     Preserve the entire existing description verbatim — never edit or delete prior review sections. Leave the task in the `review` column.

#### Range-mode

1. If the fresh review produced **zero findings**, report "clean, nothing to track" and exit. Do NOT create a tracking task.

2. Otherwise create a tracking task in the `review` column. First ensure the `#review` tag exists:

   ```json
   {"op": "list tags"}
   ```

   If no tag with `id: "review"` is present, create it:

   ```json
   {"op": "add tag", "id": "review", "name": "Review", "color": "9900cc", "description": "Ad-hoc range review tracking"}
   ```

3. Create the tracking task directly in the `review` column:

   ```json
   {"op": "add task", "title": "Review of <scope>", "description": "Scope: <range or branch>\n\n## Review Findings (YYYY-MM-DD HH:MM)\n\n### Blockers\n- [ ] ...\n\n### Warnings\n- [ ] ...", "column": "review"}
   ```

4. Tag it:

   ```json
   {"op": "tag task", "id": "<new-task-id>", "tag": "review"}
   ```

   From that point forward the tracking task is treated like any other task in review — a subsequent `/review <tracking-task-id>` follows the task-mode flow and will move it to the terminal column when all items are checked off and a fresh review is clean.

### 10. Summarize

Finish with a short report covering:

- **Mode**: task-mode (with task id) or range-mode (with scope)
- **Scope reviewed**: the effective range or branch
- **Counts**: findings by severity, e.g. "1 blocker, 3 warnings, 5 nits" (or "clean")
- **Outcome**: one of
  - task advanced from `review` to the terminal column
  - findings appended to task `<id>`; task remains in `review`
  - tracking task `<id>` created in `review`
  - range clean, no tracking task created
- Optional one-sentence overall assessment

There is no verdict label (no approve / request-changes / comment-only) — the column movement *is* the verdict.

## Examples

### Example 1: task-mode review of an implementation that just landed in review

User says: `/review 01KN2X3Y4Z5A6B7C8D9E0F1G2H`

Actions:
1. Ensure the `review` column exists (idempotent).
2. Call `{"op": "get task", "id": "01KN2X3Y4Z5A6B7C8D9E0F1G2H"}` to read the task body and scope the diff to the referenced range.
3. Call `{"op": "get changes"}` to auto-detect, read every changed file, and apply the six examination layers (plus RUST_REVIEW.md for a Rust change).
4. Fresh review produces zero findings and all prior `- [ ]` items from earlier review sections are now `- [x]`.
5. Move the task to `done` via `{"op": "move task", "id": "01KN2X3Y4Z5A6B7C8D9E0F1G2H", "column": "done"}`.

Result: Task advances from `review` to `done`. The column move is the verdict — no new findings appended, prior history preserved.

### Example 2: range-mode review with findings

User says: `/review the last 4 commits`

Actions:
1. Ensure the `review` column exists.
2. `review` column is empty, so enter range-mode. Call `{"op": "get changes", "range": "HEAD~4..HEAD"}`.
3. For each changed file, use `{"op": "get diff", "left": "src/server.rs@HEAD~4", "right": "src/server.rs"}` for semantic diffs.
4. Layered review produces 1 blocker (missing auth check) and 2 nits.
5. Ensure the `#review` tag exists via `{"op": "list tags"}` (create it if absent).
6. Create a tracking task in the `review` column: `{"op": "add task", "title": "Review of HEAD~4..HEAD", "description": "Scope: HEAD~4..HEAD\n\n## Review Findings (2026-04-24 14:08)\n\n### Blockers\n- [ ] `src/server.rs:42` — Missing auth check on /admin handler. Add `require_admin(&req)?` before the dispatch.\n\n### Nits\n- [ ] ...", "column": "review"}`.
7. Tag it: `{"op": "tag task", "id": "<new-id>", "tag": "review"}`.

Result: A single tracking task in `review` captures all findings as a GFM checklist. Subsequent `/review <new-id>` follows task-mode — moves to `done` once everything is checked off and a re-review is clean.

## Rules

- **Facts over opinions.** Technical arguments beat personal preference.
- **Review the change, not the whole file.** Flag pre-existing issues only if the change makes them worse.
- **Don't block on style.** Defer to formatters. Accept the author's style if no convention exists.
- **Be specific and actionable.** "This function is confusing" is not enough — say what's confusing and what to do about it.
- **One concern per checklist item.** Don't bundle unrelated issues into a single bullet.
- **No per-finding tasks.** Findings are checklist items on the source task (task-mode) or on a single tracking task (range-mode). The `review-finding` tag from the old workflow is retired — do not create it or reuse it.
- **Preserve history on re-run.** Always append new dated `## Review Findings` sections. Never edit or delete prior sections, and never flip checkboxes yourself — the user (or the implementer picking up the task) owns the check marks.
- **Skip gitignored files and dot-directories** (`.git/`, `.vscode/`, `.skills/`) unless explicitly asked.
