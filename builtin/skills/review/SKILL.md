---
name: review
profiles:
  - kanban
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review.
agent: reviewer
license: MIT OR Apache-2.0
compatibility: Requires the `review` MCP tool (the local multi-agent review engine) and the `kanban` MCP tool (to drive tasks through the review column and capture findings). 
metadata:
  author: "swissarmyhammer"
  version: "{{version}}"
---


# Code Review

Perform a structured code review. You are a **thin driver**: detect the mode, call the right `review` op, write the returned findings onto a kanban task, and summarize. The `review` tool runs the multi-agent engine fleet — design, reuse/dead-code, correctness, tests, security, clarity, performance, language-specific checks. You do not hand-run those layers; the engine does.

Here is what the user provided: 
$ARGUMENTS


## Guidelines

{% include "_partials/review-column" %}

## The `review` tool

The engine is op-dispatched (verb + noun). Each `review` op returns a `ReviewReport`:

- `markdown` — a dated `## Review Findings (YYYY-MM-DD HH:MM)` section: one flat GFM checklist ordered by `file:line`. Review is binary pass/fail — there is no graded severity. Write it onto the task verbatim.
- `counts` — `{ findings, confirmed, refuted }`. Use it for the summary.

| Op | Scope | When |
|----|-------|------|
| `{"op": "review working"}` | Uncommitted changes vs `HEAD` | The everyday default. |
| `{"op": "review sha", "sha": "<commit-or-range>"}` | The changes in/since a commit or range (e.g. `HEAD~4..HEAD`, `abc123..HEAD`) | A commit, range, or "since" hint. |
| `{"op": "review file", "path": "<path-or-glob>"}` | An explicit file path or glob | A specific file or set of files. |

### Passthrough modifiers

Every `review` op accepts two optional modifiers:

- **`validators`** — an array naming a subset of validators to run (defaults to every matching validator). Use it when the user wants a narrowed review — e.g. "review just duplication" → pass the duplication validator's name in `validators`.
- **`backend`** — `session` (the remote default) or `local`. Pass `"local"` when the user says "review locally" / wants the in-process Llama backend.

```json
{"op": "review working", "validators": ["duplication"]}
{"op": "review sha", "sha": "HEAD~4..HEAD", "backend": "local"}
```

There are no "dimensions" — that concept is gone. Scope is the op (`working`/`sha`/`file`); narrowing is `validators`; the backend is `backend`.

## Process

### 1. Ensure the review column

Idempotent — use the partial above. Run every time.

### 2. Determine the mode

| Invocation | Mode |
|------------|------|
| `/review <task-id>` | **task-mode** on that task |
| `/review <task-id> <sha-or-range>` | **task-mode** on that task, scoped to `<sha-or-range>` |
| Bare `/review` with tasks in `review` column | **task-mode** on the **oldest** review task |
| Bare `/review` with `review` empty | **range-mode** on the current branch's changes |
| `/review HEAD~4..HEAD`, `/review since abc123`, `/review feature-branch` | **range-mode** on that range/branch |

Bare `/review` check:

```json
{"op": "list tasks", "column": "review"}
```

If any exist, pick the oldest (lowest ordinal / earliest created) for task-mode.

**Note:** `/implement` leaves a finished task in `doing`, not `review` — it never parks tasks in `review`. So bare `/review` won't auto-target a task that was just implemented; pass `/review <id>` to target it explicitly. Orchestrators like `/finish` always pass the id (and usually a sha), so they're unaffected.

### 3. Run the engine

The chosen op decides the scope. Pass through `validators` / `backend` when the user asked to narrow or to run locally.

**Task-mode** — read the task first:

```json
{"op": "get task", "id": "<id>"}
```

Pick the scope by this precedence:

| Condition | Call |
|-----------|------|
| An explicit `<sha-or-range>` was passed (`/review <id> <sha>`) | `{"op": "review sha", "sha": "<sha-or-range>"}` |
| The description has a commit/range/branch hint | `{"op": "review sha", "sha": "<range>"}` |
| Otherwise | `{"op": "review working"}` |

An explicit `<sha-or-range>` argument wins over everything else — this is how `/finish` asks for a review scoped to the just-committed checkpoint delta (e.g. `/review <id> HEAD~1..HEAD`), so each pass reviews only that iteration's change, never the whole accumulated task diff. Findings still land on `<id>` (task-mode) — the sha only narrows the scope, it does not turn this into range-mode.

**Range-mode**:

| User says | Call |
|-----------|------|
| `/review` (review column empty) | `{"op": "review working"}` |
| `/review the last 4 commits` | `{"op": "review sha", "sha": "HEAD~4..HEAD"}` |
| `/review since abc123` | `{"op": "review sha", "sha": "abc123..HEAD"}` |
| `/review abc123..def456` | `{"op": "review sha", "sha": "abc123..def456"}` |
| `/review feature-branch` | `{"op": "review sha", "sha": "feature-branch"}` |
| `/review src/auth.rs` or a glob | `{"op": "review file", "path": "<path-or-glob>"}` |

Take the report's `markdown` (the dated `## Review Findings (...)` section) and `counts`. You do not read files or run layers yourself — the engine fleet did, including any language-specific checks (now validators).

### 4. Apply findings

Never create one kanban task per finding. Findings = checklist items on a host task — the task being reviewed (task-mode) or a single tracking task (range-mode). The engine's `markdown` is already the dated section; write it in per the contract below.

#### Task-mode

1. Re-read the target task (already have it from step 3): `{"op": "get task", "id": "<id>"}`.

2. If not already in `review`, move it there now — **this is the only path a task takes into `review`**:

   ```json
   {"op": "move task", "id": "<id>", "column": "review"}
   ```

   Implement leaves finished tasks in `doing` (it never moves them to `review`), so this is a real `doing → review` move on the first review pass, and a no-op on re-reviews once the task is already in `review`.

3. Parse the description for prior `## Review Findings (...)` sections; note whether every `- [ ]` has been flipped to `- [x]`.

4. Outcome (use the engine's `counts` to decide "zero new findings"):
   - **Zero new findings AND every prior item checked** → move to terminal column:

     ```json
     {"op": "move task", "id": "<id>", "column": "done"}
     ```

     Leave description history intact.

   - **New findings OR any prior item still unchecked** → append the report's `markdown` (a new dated `## Review Findings (YYYY-MM-DD HH:MM)` section), write it back:

     ```json
     {"op": "update task", "id": "<id>", "description": "<existing + blank line + new section>"}
     ```

     Preserve existing description verbatim — never edit or delete prior sections. Task stays in `review`.

#### Range-mode

1. Fresh review with **zero findings** (`counts` all zero) → "clean, nothing to track", exit. Do NOT create a tracking task.

2. Otherwise create a tracking task in `review`. First ensure the `#review` tag exists:

   ```json
   {"op": "list tags"}
   ```

   Missing → `{"op": "add tag", "id": "review", "name": "Review", "color": "9900cc", "description": "Ad-hoc range review tracking"}`.

3. Create directly in `review`, embedding the report's `markdown` after the scope line:

   ```json
   {"op": "add task", "title": "Review of <scope>", "description": "Scope: <range or branch>\n\n<report.markdown>", "column": "review"}
   ```

4. Tag it: `{"op": "tag task", "id": "<new-id>", "tag": "review"}`.

   A subsequent `/review <tracking-id>` follows task-mode and moves it to terminal when all items are checked and a fresh review is clean.

### 5. Summarize

- **Mode**: task-mode (with id) or range-mode (with scope)
- **Scope reviewed**: the op and its target (`review working`, `review sha HEAD~4..HEAD`, `review file src/auth.rs`)
- **Counts**: from `counts` — the findings tally ("3 findings" or "clean")
- **Outcome**: one of
  - task advanced to terminal column
  - findings appended to task `<id>`; remains in `review`
  - tracking task `<id>` created in `review`
  - range clean, no tracking task
- Optional one-sentence overall assessment

No verdict label (no approve / request-changes / comment-only) — the column movement IS the verdict.

## Examples

**Task-mode clean:** `/review 01KN2X3Y4Z5A6B7C8D9E0F1G2H`.

1. Ensure review column.
2. `get task` → read body; no range hint, so `{"op": "review working"}`.
3. Engine returns `counts` all zero, and all prior items are now `- [x]`.
4. Move to `done`.

The column move is the verdict — no findings appended, history preserved.

**Range-mode with findings:** `/review the last 4 commits`.

1. Ensure review column.
2. `review` empty → range-mode. `{"op": "review sha", "sha": "HEAD~4..HEAD"}`.
3. Engine returns `markdown` with 3 findings and the matching `counts`.
4. Ensure `#review` tag.
5. Create tracking task in `review` with `Scope: HEAD~4..HEAD` + the report's `markdown`.
6. Tag it `review`.

Subsequent `/review <new-id>` follows task-mode — moves to `done` once items are checked and a re-review is clean.

**Narrowed / local:** `/review just duplication` → `{"op": "review working", "validators": ["duplication"]}`. `/review locally` → `{"op": "review working", "backend": "local"}`.

## Rules

- **The engine is the analysis.** You drive it and record its findings; you do not re-run layers, re-read files, or second-guess the report.
- **Findings are obeyed, never declined.** A finding is an instruction: satisfy it by fixing the code. You may not dismiss a finding, and you may not edit a validator to make one disappear — both are disobedience. The one exception is findings that genuinely cannot all be satisfied (two rules that can't both hold, or one demanding code that won't compile/type-check, or fighting a deliberate documented contract like `snake_case` mirroring a backend payload or `null` required by a type): you can't obey contradictory orders, so **report it** — record it on the task and leave it in `review` (stuck) for a human to fix the rule. You do not pick a winner, edit validators, or force a verdict. Column movement remains the only verdict.
- **Facts over opinions.** The engine reports technical findings; relay them, don't editorialize.
- **One concern per checklist item.** The engine already formats this way — preserve it.
- **No per-finding tasks.** Findings = checklist items on the source task (task-mode) or a single tracking task (range-mode). The retired `review-finding` tag — don't create or reuse it.
- **Preserve history on re-run.** Always append new dated sections. Never edit or delete prior ones; never flip checkboxes yourself — the user (or the implementer picking up the task) owns the marks.
- **Column movement is the verdict.** Clean task → terminal column. Findings → stays in `review`.
