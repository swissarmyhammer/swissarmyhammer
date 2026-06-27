---
name: issue
profiles:
  - kanban
description: Turn an issue into finished work. Use when a GitHub issue URL (e.g. github.com/owner/repo/issues/123) is pasted, or when the pasted text is clearly the body of an issue (title + description, "Steps to reproduce", "Expected/Actual behavior", labels). Also fires on "/issue", "make a task from this issue", "implement this issue", "do this issue". Converts the issue into one or more kanban tasks and then drives them to done.
license: MIT OR Apache-2.0
compatibility: Requires the `kanban` MCP tool to persist tasks and delegates to the `task`, `plan`, and `finish` skills. Fetching an issue from a bare URL uses the `gh` CLI (`gh issue view`) when available, falling back to `WebFetch`; pasted issue content needs neither. Will not function on a harness that does not expose `kanban`.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Issue

Convert an issue — pasted as a **link** or as its **content** — into kanban task(s), then drive them to `done`.

$ARGUMENTS

**Orchestrator only** — does not research, write tasks, or write code itself. It normalizes the issue into text, then delegates: `/task` or `/plan` to create the card(s), `/finish` to complete them.

## 1. Recognize the input

Treat the input as an issue when it is either:

- **A GitHub issue URL** — matches `github.com/<owner>/<repo>/issues/<number>` (also accept `gh`-style `owner/repo#123`).
- **Issue-shaped text** — a title plus a body, especially with sections like *Steps to reproduce*, *Expected behavior*, *Actual behavior*, *Description*, a checklist, or label/severity lines. The user pasting a chunk of bug-report prose counts.

If it is genuinely ambiguous whether the paste is an issue or a free-form request, prefer this skill when it reads like a report of work to be done. If it is clearly a question or a discussion, this skill does not apply.

## 2. Normalize to issue text

- **URL** → fetch the issue. Prefer the authenticated, structured `gh` CLI:

  ```
  gh issue view <url-or-owner/repo#number> --json title,body,labels,comments
  ```

  If `gh` is unavailable or unauthenticated, fall back to `WebFetch` on the URL. If both fail, STOP and tell the user — do not invent the issue contents.
- **Pasted content** → use it directly; no fetch needed.

Read the comments/discussion when present — they often carry the real acceptance criteria. The normalized result is the issue's **title + body + relevant discussion** as plain text, used verbatim as the basis for the task(s).

**No GitHub coupling.** Do not store the issue URL on the card and do not write anything back to GitHub (no comments, no labels, no close). The card is created from the issue's *content*; once created it is an ordinary kanban task.

## 3. Size, then route to /task or /plan

Judge the normalized issue against the task sizing limits (one concern, 2–4 files, ≤5 subtasks):

- **Single concern, fits one card** → `/task <issue text>`. The `task` skill researches the codebase and writes one well-formed card (What / Acceptance Criteria / Tests). Capture the resulting `short_id`.
- **Multi-concern or too large for one card** (multiple independent changes, >5 natural subtasks, spans many files) → `/plan <issue text>`. The `plan` skill decomposes it into several right-sized cards linked with `depends_on`. Capture the resulting `short_id`s.

Do not hand-write task descriptions here — that is `/task` and `/plan`'s job, including the architecture research and the Task Standards template.

## 4. Finish

Hand the created id(s) to `/finish`, which loops implement → test → commit → review until each lands in `done`:

- **One card** → `/finish <short_id>` (single-task mode).
- **Several cards from /plan** → `/finish` over the batch (e.g. the project/tag the plan created), so every card is driven to done in dependency order.

Report back: which issue was ingested, the card(s) created (by `short_id`), and the final `/finish` outcome (done, or any task reported stuck).

## Constraints

- **Kanban is the single source of truth** — no TodoWrite/TaskCreate. Cards come only from `/task` or `/plan`.
- **Reuse, don't reimplement.** This skill adds recognition + normalization + chaining; all research, task-writing, and the implement loop live in the delegated skills.
- **One issue per invocation.** If several issues are pasted at once, take the most important and tell the user to re-run for the rest.

{% include "_partials/short-ids" %}
