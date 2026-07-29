---
name: review
profiles:
  - kanban
description: A code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review.
agent: reviewer
license: MIT OR Apache-2.0
compatibility: This skill needs the `review` MCP tool, the local multi-agent review engine. It also needs the `kanban` MCP tool, to move tasks through the review column and record findings.
metadata:
  author: "swissarmyhammer"
  version: "{{version}}"
---


# Code Review

Perform a structured code review. You are a **thin driver**. Detect the mode, call the correct `review` op, write the findings onto a kanban task, and summarize the result. The `review` tool runs a fleet of engines: design, reuse and dead-code, correctness, tests, security, clarity, performance, and language-specific checks. You do not run those checks yourself. The engine runs them.

Here is what the user provided: 
$ARGUMENTS


## Guidelines

{% include "_partials/review-column" %}

## The `review` tool

The engine dispatches by op: a verb plus a noun. Each `review` op returns a `ReviewReport`:

- `markdown` — a dated `## Review Findings (YYYY-MM-DD HH:MM)` section. It is one flat GFM checklist, ordered by `file:line`. A review result is either pass or fail; there is no severity scale. Write this text onto the task exactly as given.
- `counts` — `{ findings, confirmed, refuted }`. Use this for the summary.

| Op | Scope | When |
|----|-------|------|
| `{"op": "review working"}` | Uncommitted changes vs `HEAD` | Use this as the everyday default. |
| `{"op": "review sha", "sha": "<commit-or-range>"}` | The changes in or since a commit or range, for example `HEAD~4..HEAD` or `abc123..HEAD` | Use this for a commit, a range, or a "since" hint. |
| `{"op": "review file", "path": "<path-or-glob>"}` | An explicit file path or glob | A specific file or set of files. |

### Passthrough modifiers

Every `review` op accepts two optional modifiers:

- **`validators`** — a list of **validator names** to run. By default, the tool runs every validator that matches. The scope is always **one whole validator**, never a single rule. Each validator is a named bundle of rules, for example `duplication`, `security`, or a language validator like `swift`, which bundles rules such as `casing`, `optionals`, and `concurrency`. You choose validators, not individual rules; there is no rule-level filter. Use this modifier when the user wants a narrower review. For "review just duplication", pass `["duplication"]`. For "review the Swift idioms", pass `["swift"]`. To find the available names, and the rules each one bundles, call `{"op": "list validators"}`. For one validator's full rule list, call `{"op": "get validator", "name": "<name>"}`.
- **`backend`** — `session` (the remote default) or `local`. Pass `"local"` when the user says "review locally" or wants the in-process Llama backend.

```json
{"op": "review working", "validators": ["duplication"]}
{"op": "review sha", "sha": "HEAD~4..HEAD", "backend": "local"}
```

There is no "dimensions" concept; that idea is gone. The op sets the scope: `working`, `sha`, or `file`. The `validators` field narrows the scope, by whole validator, not by individual rule. The `backend` field sets the backend.

## Process

### 1. Ensure the review column

This step is idempotent. Use the partial above. Run it every time.

### 2. Determine the mode

| Invocation | Mode |
|------------|------|
| `/review <task-id>` | **task-mode** on that task |
| `/review <task-id> <sha-or-range>` | **task-mode** on that task, scoped to `<sha-or-range>` |
| Bare `/review` with tasks in `review` column | **task-mode** on the **oldest** review task |
| Bare `/review` with `review` empty | **range-mode** on the changes of the current branch |
| `/review HEAD~4..HEAD`, `/review since abc123`, `/review feature-branch` | **range-mode** on that range or branch |

Bare `/review` check:

```json
{"op": "list tasks", "column": "review"}
```

If any tasks exist, pick the oldest one for task-mode. This is the task with the lowest ordinal, or the earliest creation time.

**Note:** `/implement` leaves a finished task in `doing`, not in `review`. It never places a task in `review`. So a bare `/review` will not automatically target a task that was just implemented. Pass `/review <id>` to target it directly. Orchestrators such as `/finish` always pass the id, and usually a sha too, so this does not affect them.

### 3. Run the engine

The chosen op sets the scope. Pass through `validators` or `backend` when the user asks to narrow the review, or to run it locally.

**Task-mode.** Read the task first:

```json
{"op": "get task", "id": "<id>"}
```

Pick the scope by this precedence:

| Condition | Call |
|-----------|------|
| An explicit `<sha-or-range>` was passed (`/review <id> <sha>`) | `{"op": "review sha", "sha": "<sha-or-range>"}` |
| The description has a hint of a commit, range, or branch | `{"op": "review sha", "sha": "<range>"}` |
| Otherwise | `{"op": "review working"}` |

An explicit `<sha-or-range>` argument wins over every other rule. This is how `/finish` asks for a review scoped to the change just committed, for example `/review <id> HEAD~1..HEAD`. Each pass then reviews only that round's change, not the whole accumulated task diff. Findings still land on `<id>`, in task-mode. The sha only narrows the scope; it does not switch the mode to range-mode.

**Range-mode**:

| User says | Call |
|-----------|------|
| `/review` (review column empty) | `{"op": "review working"}` |
| `/review the last 4 commits` | `{"op": "review sha", "sha": "HEAD~4..HEAD"}` |
| `/review since abc123` | `{"op": "review sha", "sha": "abc123..HEAD"}` |
| `/review abc123..def456` | `{"op": "review sha", "sha": "abc123..def456"}` |
| `/review feature-branch` | `{"op": "review sha", "sha": "feature-branch"}` |
| `/review src/auth.rs` or a glob | `{"op": "review file", "path": "<path-or-glob>"}` |

Take the `markdown` field from the report (the dated `## Review Findings (...)` section) and the `counts` field. You do not read files or run the checks yourself. The engine fleet already did this, including any language-specific checks, now called validators.

### 4. Apply findings

Do not create one kanban task for each finding. Findings become checklist items on a host task: the task under review, in task-mode, or one tracking task, in range-mode. The `markdown` field from the engine is already the dated section. Write it in following the rule below.

#### Task-mode

1. Read the target task again. You already have it from step 3:

2. If the task is not already in `review`, move it there now. **This is the only path a task takes into `review`**:

   ```json
   {"op": "move task", "id": "<id>", "column": "review"}
   ```

   Implement leaves finished tasks in `doing`; it never moves them to `review`. So this step is a real move from `doing` to `review` on the first review pass. On a later review, once the task is already in `review`, this step does nothing.

3. Read the description for earlier `## Review Findings (...)` sections. Note whether every `- [ ]` box is now `- [x]`.

4. Decide the outcome. Use the `counts` field from the engine to decide whether there are "zero new findings":
   - **Zero new findings, and every earlier item checked.** Move the task to the terminal column:

     ```json
     {"op": "move task", "id": "<id>", "column": "done"}
     ```

     Leave the description history unchanged.

   - **New findings exist, or an earlier item is still unchecked.** Add the `markdown` from the report as a new dated `## Review Findings (YYYY-MM-DD HH:MM)` section. Write it back:

     ```json
     {"op": "update task", "id": "<id>", "description": "<existing + blank line + new section>"}
     ```

     Keep the existing description exactly as it is. Do not edit or delete earlier sections. The task stays in `review`.

#### Range-mode

1. If the fresh review has **zero findings** (`counts` all zero), report "clean, nothing to track" and stop. Do not create a tracking task.

2. Otherwise, create a tracking task in `review`. First make sure the `#review` tag exists:

   ```json
   {"op": "list tags"}
   ```

   If the tag is missing, create it: `{"op": "add tag", "id": "review", "name": "Review", "color": "9900cc", "description": "Ad-hoc range review tracking"}`.

3. Create the task directly in `review`. Put the `markdown` from the report after the scope line:

   ```json
   {"op": "add task", "title": "Review of <scope>", "description": "Scope: <range or branch>\n\n<report.markdown>", "column": "review"}
   ```

4. Tag it: `{"op": "tag task", "id": "<new-id>", "tag": "review"}`.

   A later `/review <tracking-id>` follows task-mode. It moves the task to the terminal column when all items are checked, and a fresh review is clean.

### 5. Summarize

- **Mode**: task-mode (with id) or range-mode (with scope)
- **Scope reviewed**: the op and its target (`review working`, `review sha HEAD~4..HEAD`, `review file src/auth.rs`)
- **Counts**: the findings tally from `counts`, for example "3 findings" or "clean"
- **Outcome**: one of
  - task advanced to terminal column
  - findings added to task `<id>`; the task remains in `review`
  - tracking task `<id>` created in `review`
  - range clean, no tracking task
- Optional one-sentence overall assessment

There is no verdict label, such as approve, request changes, or comment-only. The column movement is the verdict.

## Examples

**Task-mode clean:** `/review 01KN2X3Y4Z5A6B7C8D9E0F1G2H`.

1. Ensure review column.
2. Call `get task`, and read the body. There is no range hint, so call `{"op": "review working"}`.
3. Engine returns `counts` all zero, and all prior items are now `- [x]`.
4. Move to `done`.

The column move is the verdict. No findings are added, and history is preserved.

**Range-mode with findings:** `/review the last 4 commits`.

1. Ensure review column.
2. The `review` column is empty, so use range-mode: `{"op": "review sha", "sha": "HEAD~4..HEAD"}`.
3. Engine returns `markdown` with 3 findings and the matching `counts`.
4. Ensure `#review` tag.
5. Create a tracking task in `review` with `Scope: HEAD~4..HEAD` and the `markdown` from the report.
6. Tag it `review`.

A later `/review <new-id>` follows task-mode. It moves the task to `done` once the items are checked and a re-review is clean.

**Narrowed or local:** `/review just duplication` calls `{"op": "review working", "validators": ["duplication"]}`. `/review locally` calls `{"op": "review working", "backend": "local"}`.

## Rules

- **The engine performs the analysis.** You drive it and record its findings. Do not run the checks again, read the files again, or question the report.
- **You must obey every finding; you must never decline one.** A finding is an instruction. Satisfy it by fixing the code. You must not dismiss a finding. You must not edit a validator to make a finding disappear; both actions are disobedience. There is one exception: findings that truly cannot all be satisfied together. This happens when two rules conflict, when one rule demands code that will not compile or type-check, or when a rule fights a deliberate, documented contract, for example `snake_case` that mirrors a backend payload, or a `null` value required by a type. You cannot obey contradictory orders, so **report the conflict**. Record it on the task, and leave the task in `review` as stuck, for a person to fix the rule. Do not pick a winner, edit a validator, or force a verdict. Column movement remains the only verdict.
- **Fix the root cause, not only the cited line.** A finding names one instance of a cause. Satisfy it by removing that cause across the whole file, so a re-review of the file finds zero more instances. Do not patch only the cited line. A review result is binary, like a test suite: any open finding means the work is not done, no matter how small it looks. There is no severity tier that makes a finding optional. Every finding is mandatory.
- **Report facts, not opinions.** The engine reports technical findings. Relay them. Do not add your own opinion.
- **Do not ask to refactor existing tests. This exception overrides every other rule.** Do not raise, record, or relay any finding about *changing test code that already existed*. This includes refactoring, removing duplication, restructuring, renaming, changing docstrings, or restyling test code, even when a validator flags it, for example the duplication, complexity, missing-docs, reuse, naming, or function-length validator. **Drop the finding.** Adding a *new* regression test for the change under review is fine, and expected. Rewriting tests that were already in the repository is out of scope. The reason: test refactoring is not the task. It wastes the implement loop on churn. Critically, rewriting an existing test file can collide with the upstream test suite that grades the change, and can turn a correct fix into a broken merge. Only an *explicit* request from the user to refactor tests lifts this exception.
- **Keep one concern in each checklist item.** The engine already formats findings this way. Keep this format.
- **Do not create one task for each finding.** Findings become checklist items on the source task, in task-mode, or on one tracking task, in range-mode. The `review-finding` tag is retired. Do not create it or reuse it.
- **Keep the history on every re-run.** Always add new dated sections. Do not edit or delete earlier sections. Do not check or uncheck a box yourself; the user, or the implementer who picks up the task, owns the checkmarks.
- **Column movement is the verdict.** A clean task moves to the terminal column. A task with findings stays in `review`.
