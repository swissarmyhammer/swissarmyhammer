---
name: review
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

{% include "_partials/review-column" %}

## Guidelines


- **The engine is the analysis.** You drive it and record its findings; you do not re-run layers, re-read files, or second-guess the report.
- **Facts over opinions.** The engine reports technical findings; relay them, don't editorialize.
- **One concern per checklist item.** The engine already formats this way — preserve it.
- **No per-finding tasks.** Findings = checklist items on the source task (task-mode) or a single tracking task (range-mode). The retired `review-finding` tag — don't create or reuse it.
- **Preserve history on re-run.** Always append new dated sections. Never edit or delete prior ones; never flip checkboxes yourself — the user (or the implementer picking up the task) owns the marks.
- **Column movement is the verdict.** Clean task → terminal column. Findings → stays in `review`.

{% include "_partials/findings-are-requirements" %}

## The `review` tool

The engine is op-dispatched (verb + noun). Each `review` op returns a `ReviewReport`:

- `markdown` — a dated `## Review Findings (YYYY-MM-DD HH:MM)` section: one flat GFM checklist ordered by `file:line`. Each item reads ``- [ ] `file:line` `set/rule` — claim. suggestion.``, so the item names the validator set and the rule that produced it and the reader opens that rule without searching. Review is binary pass/fail — there is no graded severity. Write it onto the task verbatim.
- `counts` — `{ findings, confirmed, refuted }`. Use it for the summary.

| Op | Scope | Reviews | When |
|----|-------|---------|------|
| `{"op": "review working"}` | Uncommitted changes vs `HEAD` | **the diffs only** | The everyday default. |
| `{"op": "review sha", "sha": "<commit-or-range>"}` | The changes in/since a commit or range (e.g. `HEAD~4..HEAD`, `abc123..HEAD`) | **the diffs only** | A commit, range, or "since" hint. |
| `{"op": "review file", "path": "<path-or-glob>"}` | An explicit file path or glob | **the whole file** | A specific file or set of files. |

### What each op reviews

The op is the whole of it. There is no argument, no modifier and no flag that
picks between the two — asking about the working tree or a sha is asking about
a CHANGE, and asking about a path or a glob is asking about FILES.

#### Under `review working` and `review sha` — report findings ONLY on the diffs

Report a finding **only** on a line the change added or modified. Report
**nothing** on any other line of the file.

This holds no matter what else the file contains. A real defect on an unchanged
line is **not a finding** in a diff op. Neither is a defect a validator flags on
an unchanged line. Neither is one you are certain about. If the change did not
write that line, it is not under review, and you do not report it.

The rest of the file is given to you for one reason: so you can judge the
changed lines correctly. Read it, use it, and report nothing from it.

The engine enforces this — a finding off the changed lines is refuted before it
reaches the report — so a finding you raise there is not merely dropped, it is
wasted work.

#### Under `review file` — report findings anywhere in the named files

Every line of each named file is under review. The caller asked about those
files, so answer about all of them.

#### The two words the engine renders in every prompt

- **REVIEW** — the subject. A finding must land here.
- **CONSIDER** — context. Read it to judge the subject; never report it.

Under a diff op, REVIEW is the added and modified lines and CONSIDER is
everything else. Under a file op, REVIEW is the whole of each named file.

Two consequences for you as the driver:

- `/finish` always passes a sha, so every finish review reviews that
  iteration's diffs and nothing else. You never need `git blame` to decide
  whether a finding belongs to the change — the engine has already answered
  that.
- `/review <path>` on a test file is a legitimate and wanted request. It
  reviews that file whole, like any other file. Relay what it finds.

### Passthrough modifiers

Every `review` op accepts two optional modifiers:

- **`validators`** — an array of **validator names** to run (defaults to every matching validator). Scoping is **whole-validator**, never per-rule: each validator is a named bundle of rules — `duplication`, `security`, or a language validator like `swift` (which bundles `casing`, `optionals`, `concurrency`, … as its rules). You pick validators, not individual rules; there is no rule-level filter. Use it when the user wants a narrowed review — "review just duplication" → `["duplication"]`; "review the Swift idioms" → `["swift"]`. To discover the available names (and the rules each bundles), call `{"op": "list validators"}`, or `{"op": "get validator", "name": "<name>"}` for one validator's full rule list.
- **`backend`** — `session` (the remote default) or `local`. Pass `"local"` when the user says "review locally". `local` runs one worker at a time, which suits a single in-process model.

```json
{"op": "review working", "validators": ["duplication"]}
{"op": "review sha", "sha": "HEAD~4..HEAD", "backend": "local"}
```

There are no "dimensions" — that concept is gone. Scope is the op (`working`/`sha`/`file`); narrowing is `validators` (whole validators, not individual rules); the backend is `backend`.

## Process

### Determine the mode

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

### Run the engine

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

### Apply findings

Never create new kanban tasks for findings. 
Findings = checklist items on a host task — the task being reviewed (task-mode) or a single tracking task (range-mode). The engine's `markdown` is already the dated section; write it in per the contract below.

#### Task-mode

1. Re-read the target task (already have it from step 3): `{"op": "get task", "id": "<id>"}`.

2. If not already in `review`, move it there now.

   ```json
   {"op": "move task", "id": "<id>", "column": "review"}
   ```

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

### Summarize

{% include "_partials/step-record" %}

Review reports `clean`, `findings`, or `stuck`. The evidence is the `counts` and the `file:line` list.

```
step: review
outcome: findings
evidence: 2 findings — crates/kanban/src/tag.rs:88, crates/kanban/src/tag.rs:140
task: ^rc9rb4g
```

Report `stuck` only for a contradiction you cannot obey (see the Guidelines above). Record the conflict on the task, leave the task in `review`, and stop.

Write the same record as a comment on the host task. The two writes have different jobs: the dated `## Review Findings` section is the state the implementer must act on, and the comment is the history of this pass. A range-mode review that is clean has no task — return `task: none` and skip the card block below.

{% include "_partials/card-report" %}

After the block, add these facts:

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
