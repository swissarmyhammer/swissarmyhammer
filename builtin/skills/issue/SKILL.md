---
name: issue
profiles:
  - kanban
description: Turn an issue into finished work. Use this skill when a user pastes a GitHub issue URL (for example, github.com/owner/repo/issues/123). Also use it when the pasted text is clearly the body of an issue. An issue body has a title and a description. It may have "Steps to reproduce", "Expected/Actual behavior", or labels. This skill also fires on "/issue", "make a task from this issue", "implement this issue", and "do this issue". The skill converts the issue into one or more kanban tasks. Then it drives the tasks to done.
license: MIT OR Apache-2.0
compatibility: This skill requires the `kanban` MCP tool to store tasks. It delegates to the `task`, `plan`, and `finish` skills. To fetch an issue from a bare URL, the skill uses the `gh` CLI (`gh issue view`) when available. If `gh` is not available, the skill falls back to `WebFetch`. Pasted issue content needs neither tool. The skill will not work on a harness that does not expose `kanban`.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Issue

Convert an issue into kanban task(s), then drive them to `done`. Paste the issue as a **link** or as its **content**.

$ARGUMENTS

**Orchestrator only.** This skill does not research, write tasks, or write code itself. It normalizes the issue into text. Then it delegates the work: `/task` or `/plan` creates the card(s); `/finish` completes them.

## 1. Recognize the input

Treat the input as an issue in either of these two cases:

- **A GitHub issue URL** — matches `github.com/<owner>/<repo>/issues/<number>` (also accept `gh`-style `owner/repo#123`).
- **Issue-shaped text** — a title plus a body, often with sections like *Steps to reproduce*, *Expected behavior*, *Actual behavior*, *Description*, a checklist, or label/severity lines. A pasted chunk of bug-report prose also counts.

The paste may be genuinely ambiguous between an issue and a free-form request. In that case, prefer this skill when the text reads like a report of work to do. Do not use this skill when the text is clearly a question or a discussion.

## 2. Normalize to issue text

- **URL** → fetch the issue. Prefer the authenticated, structured `gh` CLI:

  ```
  gh issue view <url-or-owner/repo#number> --json title,body,labels,comments
  ```

  If `gh` is not available or not authenticated, fall back to `WebFetch` on the URL. If both fail, stop. Tell the user. Do not invent the issue contents.
- **Pasted content** → use it directly. Do not fetch it.

Read the comments and the discussion when present. They often carry the real acceptance criteria. The normalized result is the issue's **title, body, and relevant discussion** as plain text. Use this text verbatim as the basis for the task(s).

**No GitHub coupling.** Do not store the issue URL on the card. Do not write anything back to GitHub — no comments, no labels, no close. The card comes from the issue's *content*; once created it is an ordinary kanban task.

## 3. Size, then route to /task or /plan

Judge the normalized issue against the task sizing limits: one concern, 2–4 files, and 5 or fewer subtasks.

- **Single concern, fits one card** → `/task <issue text>`. The `task` skill researches the codebase and writes one well-formed card (What / Acceptance Criteria / Tests). Capture the resulting `short_id`.
- **Multi-concern or too large for one card** (multiple independent changes, >5 natural subtasks, spans many files) → `/plan <issue text>`. The `plan` skill splits it into several right-sized cards linked with `depends_on`. Capture the resulting `short_id`s.

Do not hand-write task descriptions here. That is the job of `/task` and `/plan`, including the architecture research and the Task Standards template.

## 4. Finish

Hand the created id(s) to `/finish`. It loops implement → test → commit → review until each task lands in `done`:

- **One card** → `/finish <short_id>` (single-task mode).
- **Several cards from /plan** → `/finish` over the batch (for example, the project or tag the plan created), so every card is driven to done in dependency order.

Report back three things: which issue you ingested, the card(s) you created (by `short_id`), and the final `/finish` outcome — done, or any task reported stuck.

## Constraints

- **Kanban is the single source of truth** — do not use TodoWrite or TaskCreate. Cards come only from `/task` or `/plan`.
- **Reuse; do not reimplement.** This skill adds recognition, normalization, and chaining; all research, task-writing, and the implement loop live in the delegated skills.
- **One issue per invocation.** If the user pastes several issues at once, take the most important one and tell the user to re-run this skill for the rest.

{% include "_partials/short-ids" %}
