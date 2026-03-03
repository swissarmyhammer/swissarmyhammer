---
name: review
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review. Reviews produce verbose output — automatically delegates to a reviewer subagent.
context: fork
agent: reviewer
metadata:
  author: "swissarmyhammer"
  version: "3.0"
---

# Code Review

Perform a structured code review on the current changes.

## Process

### 1. Get the Changes

Use `git` with `op: "get changes"` to get the list of files changed on the current branch. This returns committed changes since diverging from the parent branch plus any uncommitted work.

If a specific branch was requested, pass it as the `branch` parameter:

```json
{"op": "get changes", "branch": "feature-branch"}
```

Read the full content of every changed file — diffs alone lack context. Understand the **purpose** of the change before reviewing (PR description, commit messages, kanban cards).

### 2. Layered Examination

Review in progressive layers. Do not skip layers — each catches different classes of problems.

**Layer 1: Design and Architecture** — Does the change fit the system? Appropriate abstractions? Over-engineering? Does it belong in this codebase or in a library?

**Layer 2: Functionality and Correctness** — Does the code do what the author intended? Is that good for users? Edge cases: empty inputs, nulls, boundary values, error conditions. Off-by-one errors, incorrect boolean logic, missing early returns. Concurrency: race conditions, deadlocks, shared mutable state.

**Layer 3: Tests** — Tests for new/changed behavior? Do they verify behavior, not implementation? Would they fail if the code were broken? Edge cases covered? Mocks only at system boundaries?

**Layer 4: Security** — Input validated and sanitized? Injection risks (SQL, command, XSS, template)? Secrets handled safely? Auth checks in place? Error messages safe?

**Layer 5: Naming, Clarity, Simplicity** — Names descriptive without being verbose? Code understandable without explanation? Comments explain "why", not "what"? Stale comments or TODOs?

**Layer 6: Performance** (when relevant) — O(n^2) or worse on large data? Unnecessary allocations in hot paths? N+1 queries? Resource cleanup in all paths?

### 3. Review Every Line

Look at every line of changed code. If code is hard to understand, that is itself a finding.

### 4. Apply Language-Specific Guidelines

Read the matching resource file bundled with this skill:

| Language | File |
|----------|------|
| Rust | [RUST_REVIEW.md](./RUST_REVIEW.md) |
| Dart / Flutter | [DART_FLUTTER_REVIEW.md](./DART_FLUTTER_REVIEW.md) |
| Python | [PYTHON_REVIEW.md](./PYTHON_REVIEW.md) |
| JavaScript / TypeScript | [JS_TS_REVIEW.md](./JS_TS_REVIEW.md) |

If the project uses multiple languages, apply all relevant sections. Language-specific findings follow the same severity levels.

### 5. Produce Findings

Organize findings by severity. Each finding must be specific and actionable.

| Severity | Meaning | Action |
|----------|---------|--------|
| **blocker** | Correctness bug, security vulnerability, data loss risk | Must fix |
| **warning** | Design problem, missing test, performance concern | Should fix |
| **nit** | Style preference, optional improvement | Optional |

Each finding: **where** (file:line), **what**, **why** (skip for nits), and **suggestion** when non-obvious.

### 6. Capture Findings as Kanban Cards

Initialize the board:

```json
{"op": "init board"}
```

Create tags for review severities:

```json
{"op": "add tag", "id": "review-finding", "name": "Review Finding", "color": "9900cc", "description": "Code review finding"}
```

```json
{"op": "add tag", "id": "blocker", "name": "Blocker", "color": "ff0000", "description": "Must fix before merge"}
```

```json
{"op": "add tag", "id": "warning", "name": "Warning", "color": "ff8800", "description": "Should fix"}
```

Each **blocker** and **warning** becomes a kanban card. Always include the `review-finding` tag so the implement workflow can pick up review cards:

```json
{"op": "add task", "title": "<concise description>", "description": "<file:lines>\n\n<what and why>\n\n<suggestion>", "tags": ["review-finding", "blocker"]}
```

Add subtasks for each fix step. Every card MUST include a verification subtask.

Do NOT create cards for nits — report them in the summary.

### 7. Summarize

- One-sentence overall assessment
- Count of findings by severity (e.g., "1 blocker, 3 warnings, 5 nits")
- List of kanban cards created with their IDs and titles
- Verdict: **approve**, **request changes**, or **comment only**
  - **Approve**: no blockers, warnings are minor or acceptable
  - **Request changes**: blockers exist, or warnings are serious enough to address first
  - **Comment only**: not enough context to approve or reject
- Nits listed inline

## Rules

- **Facts over opinions.** Technical arguments beat personal preference.
- **Review the change, not the whole file.** Flag pre-existing issues only if the change makes them worse.
- **Don't block on style.** Defer to formatters. Accept the author's style if no convention exists.
- **Be specific and actionable.** "This function is confusing" is not enough.
- **One concern per finding.** Don't bundle unrelated issues.
- **Skip gitignored files and dot-directories** (`.git/`, `.vscode/`, `.skills/`) unless explicitly asked.
