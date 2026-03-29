---
name: review
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review. Reviews produce verbose output — automatically delegates to a reviewer subagent.
metadata:
  author: "swissarmyhammer"
  version: "0.11.0"
---

## Project Detection

To discover project types, build commands, and language-specific guidelines for this workspace, call the code_context tool:

```json
{"op": "detect projects"}
```

**Call this early in your session** to understand the project structure before making changes. The guidelines returned are authoritative — follow them for test commands, build commands, and formatting.

## Code Quality

- Write clean, readable code that follows existing patterns in the codebase
- Prefer simple, obvious solutions over clever ones
- Make minimal changes to achieve the goal - avoid unnecessary refactoring
- Don't add features, abstractions, or "improvements" beyond what was asked

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

- Every function needs a docstring explaining what it does
- Document parameters, return values, and errors
- Update existing documentation if your changes make it stale
- Inline comments explain "why", not "what"

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees


# Code Review

Perform a structured code review on the current changes.

## Process

### 1. Get the Changes

Use `git` with `op: "get changes"` to get the list of changed files.

**Determine the scope from the user's request:**

| User says | `get changes` call |
|-----------|-------------------|
| `/review` (nothing else) | `{"op": "get changes"}` — auto-detects branch or defaults to last commit on main |
| `/review the last 4 commits` | `{"op": "get changes", "range": "HEAD~4..HEAD"}` |
| `/review since abc123` | `{"op": "get changes", "range": "abc123..HEAD"}` |
| `/review abc123..def456` | `{"op": "get changes", "range": "abc123..def456"}` |
| `/review feature-branch` | `{"op": "get changes", "branch": "feature-branch"}` |

Parse the user's natural language for commit count ("last N commits"), commit refs, or ranges, and map to the `range` parameter. If the user mentions a branch name instead, use `branch`. When in doubt, omit both and let the tool auto-detect.

Read the full content of every changed file — diffs alone lack context. Understand the **purpose** of the change before reviewing (PR description, commit messages, kanban cards).

When a `range` was used (explicit or auto-defaulted), use `get diff` with `file@<start-ref>` and `file@<end-ref>` syntax to get semantic diffs for each changed file. For example, to diff a file across a range:

```json
{"op": "get diff", "left": "src/main.rs@HEAD~4", "right": "src/main.rs"}
```

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

Severity is informational — all findings become kanban cards.

| Severity | Meaning |
|----------|---------|
| **blocker** | Correctness bug, security vulnerability, data loss risk |
| **warning** | Design problem, missing test, performance concern |
| **nit** | Style preference, minor improvement |

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

Every finding — blocker, warning, and nit — becomes a kanban card tagged `review-finding`. No finding is too small to track.

```json
{"op": "add task", "title": "<concise description>", "description": "<file:lines>\n\n<what and why>\n\n<suggestion>", "tags": ["review-finding"]}
```

Add subtasks for each fix step. Every card MUST include a verification subtask.

### 7. Summarize

- One-sentence overall assessment
- **Scope reviewed**: branch name and parent, or the revision range used (e.g. "Reviewed `HEAD~4..HEAD` on main")
- Count of findings by severity (e.g., "1 blocker, 3 warnings, 5 nits")
- List of kanban cards created with their IDs and titles
- Verdict: **approve**, **request changes**, or **comment only**
  - **Approve**: no blockers, warnings are minor or acceptable
  - **Request changes**: blockers exist, or warnings are serious enough to address first
  - **Comment only**: not enough context to approve or reject
- All findings listed as kanban cards

## Rules

- **Facts over opinions.** Technical arguments beat personal preference.
- **Review the change, not the whole file.** Flag pre-existing issues only if the change makes them worse.
- **Don't block on style.** Defer to formatters. Accept the author's style if no convention exists.
- **Be specific and actionable.** "This function is confusing" is not enough.
- **One concern per finding.** Don't bundle unrelated issues.
- **Skip gitignored files and dot-directories** (`.git/`, `.vscode/`, `.skills/`) unless explicitly asked.
