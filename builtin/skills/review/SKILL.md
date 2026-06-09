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

{% include "_partials/delegate-to-subagent" %}

## Guidelines

{% include "_partials/review-column" %}

## The `review` tool

The engine is op-dispatched (verb + noun). Each `review` op returns a `ReviewReport`:

- `markdown` — a dated `## Review Findings (YYYY-MM-DD HH:MM)` section, organized by severity, already formatted as a GFM checklist. Write it onto the task verbatim.
- `counts` — `{ blockers, warnings, nits, confirmed, refuted }`. Use it for the summary.

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
| Bare `/review` with tasks in `review` column | **task-mode** on the **oldest** review task |
| Bare `/review` with `review` empty | **range-mode** on the current branch's changes |
| `/review HEAD~4..HEAD`, `/review since abc123`, `/review feature-branch` | **range-mode** on that range/branch |

Bare `/review` check:

```json
{"op": "list tasks", "column": "review"}
```

If any exist, pick the oldest (lowest ordinal / earliest created) for task-mode.

### 3. Run the engine

The chosen op decides the scope. Pass through `validators` / `backend` when the user asked to narrow or to run locally.

**Task-mode** — read the task first:

```json
{"op": "get task", "id": "<id>"}
```

Derive the scope from any range hint in the description (commit range, branch, "since" ref):

| Task body has | Call |
|---------------|------|
| A commit/range/branch hint | `{"op": "review sha", "sha": "<range>"}` |
| No range hint | `{"op": "review working"}` |

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

2. If not in `review`, move it there first (covers manual `/review <id>` on a task still in `todo`/`doing`):

   ```json
   {"op": "move task", "id": "<id>", "column": "review"}
   ```

   No-op when it came from `implement` already in `review`.

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
- **Counts**: from `counts` — by severity ("1 blocker, 3 warnings, 5 nits" or "clean")
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
3. Engine returns `markdown` with 1 blocker + 2 nits and the matching `counts`.
4. Ensure `#review` tag.
5. Create tracking task in `review` with `Scope: HEAD~4..HEAD` + the report's `markdown`.
6. Tag it `review`.

Subsequent `/review <new-id>` follows task-mode — moves to `done` once items are checked and a re-review is clean.

**Narrowed / local:** `/review just duplication` → `{"op": "review working", "validators": ["duplication"]}`. `/review locally` → `{"op": "review working", "backend": "local"}`.

## Rules

- **The engine is the analysis.** You drive it and record its findings; you do not re-run layers, re-read files, or second-guess the report.
- **Facts over opinions.** The engine reports technical findings; relay them, don't editorialize.
- **One concern per checklist item.** The engine already formats this way — preserve it.
- **No per-finding tasks.** Findings = checklist items on the source task (task-mode) or a single tracking task (range-mode). The retired `review-finding` tag — don't create or reuse it.
- **Preserve history on re-run.** Always append new dated sections. Never edit or delete prior ones; never flip checkboxes yourself — the user (or the implementer picking up the task) owns the marks.
- **Column movement is the verdict.** Clean task → terminal column. Findings → stays in `review`.
