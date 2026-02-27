---
name: review
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review. Performs structured, layered code review on changed files. Reviews are thorough and produce verbose output — consider delegating to a review-focused agent to keep the main context clean.
metadata:
  author: "swissarmyhammer"
  version: "1.1"
---

# Code Review

Perform a structured code review on the current changes. The review examines **what changed** against a base branch (or working tree), applies layered analysis, and produces actionable findings organized by severity.

## The Standard

**Approve changes that improve overall code health, even if they aren't perfect.**

There is no "perfect" code — only better code. The reviewer seeks continuous improvement, not perfection. A change that improves maintainability, readability, or correctness should not be blocked for polish. But never approve changes that degrade code health.

Prefix optional suggestions with "Nit:" to distinguish must-fix issues from nice-to-haves.

## Process

### 1. Gather Context

Determine what is being reviewed:

- Use `git_changes` to get the diff of changed files against the base branch (usually `main`)
- If the user specified a PR, branch, or commit range, use that scope
- If the user pointed at specific files, scope the review to those files
- Read the full content of every changed file — diffs alone lack context

Understand the **purpose** of the change before reviewing the code. Look for:
- PR description, commit messages, or linked issues
- Kanban card descriptions if available
- Ask the user if the intent is unclear

### 2. Layered Examination

Review in progressive layers. Do not skip layers — each catches different classes of problems.

#### Layer 1: Design and Architecture

The most important layer. Ask:

- Does this change belong in this codebase, or in a library?
- Does it integrate well with the existing system?
- Does the approach fit the broader architecture or introduce unnecessary coupling?
- Are abstractions appropriate — not too generic, not too specific?
- Is there over-engineering? (Code more generic than needed, or functionality not yet required.)
- Are responsibilities in the right place?

#### Layer 2: Functionality and Correctness

- Does the code do what the author intended?
- Is what the author intended good for the users of this code?
- Think about edge cases: empty inputs, nulls, boundary values, error conditions
- Look for off-by-one errors, incorrect boolean logic, missing early returns
- For concurrent code: are there race conditions, deadlocks, or shared mutable state issues?
- Are error paths handled? What happens when things go wrong?

#### Layer 3: Tests

- Are there tests for the new/changed behavior?
- Do the tests actually verify the behavior, or just exercise the code?
- Would the tests fail if the code were broken? Watch for tests that always pass.
- Are edge cases covered?
- Are tests testing behavior through public interfaces, not implementation details?
- Are mocks used only at system boundaries (external APIs, databases, time)?
- Tests are also code — reject unnecessary complexity in tests

#### Layer 4: Security

- Is user input validated and sanitized?
- Are there injection risks (SQL, command, XSS, template)?
- Are secrets, credentials, or tokens handled safely?
- Are authentication and authorization checks in place where needed?
- Are error messages safe (no stack traces, internal paths, or sensitive data leaked)?
- Are dependencies trustworthy and up to date?

#### Layer 5: Naming, Clarity, and Simplicity

- Are names descriptive without being verbose?
- Can the code be understood without explanation?
- Is there unnecessary complexity? ("Too complex" = can't be understood quickly by code readers, or developers will likely introduce bugs when modifying it.)
- Do comments explain **why**, not **what**? If a comment explains what the code does, the code should be simplified instead.
- Are there stale comments, obsolete TODOs, or misleading documentation?

#### Layer 6: Performance (When Relevant)

Only flag performance issues that matter in context. Do not micro-optimize.

- Are there O(n^2) or worse algorithms on potentially large data?
- Are there unnecessary allocations, copies, or repeated computations in hot paths?
- Are database queries efficient? Watch for N+1 queries.
- Are large results paginated?
- Is there resource cleanup in all paths (connections, file handles, locks)?

### 3. Review Every Line

Look at every line of changed code. Some things (generated code, data files) can be scanned, but never assume human-written code is correct without reading it.

If the code is hard to understand, that is itself a finding — code should be clear to its readers.

### 4. Produce Findings

Organize findings by severity. Each finding must be specific and actionable.

#### Severity Levels

| Severity | Meaning | Action |
|----------|---------|--------|
| **blocker** | Correctness bug, security vulnerability, data loss risk, or broken functionality. Must be fixed before merge. | Must fix |
| **warning** | Design problem, missing test, performance concern, or maintainability issue. Should be fixed, but judgment call on timing. | Should fix |
| **nit** | Style preference, optional improvement, minor naming suggestion. Can be ignored. | Optional |

#### Finding Format

For each finding, state:

1. **Where**: file path and line number(s)
2. **What**: concise description of the issue
3. **Why**: why it matters (skip for nits)
4. **Suggestion**: how to fix it, when non-obvious

### 5. Capture Findings as Kanban Cards

Review findings are work items. Capture them on the kanban board so they can be tracked and acted on.

#### Initialize the board

Use `kanban` with `op: "init board"`, `name: "<workspace name>"`. If the board already exists this is a no-op.

#### Set up review tags

Ensure tags exist for review severities. Create them if they don't already exist:

- `kanban` with `op: "add tag"`, `id: "blocker"`, `name: "Blocker"`, `color: "ff0000"`, `description: "Must fix before merge"`
- `kanban` with `op: "add tag"`, `id: "warning"`, `name: "Warning"`, `color: "ff8800"`, `description: "Should fix"`

#### Create cards for blockers and warnings

Each **blocker** and **warning** becomes its own kanban card. Do NOT create cards for nits — report them in the summary but they are not tracked work.

For each blocker or warning, use `kanban` with:
- `op: "add task"`
- `title: "<concise description>"`
- `description: "<file path and line numbers>\n\n<what is wrong and why it matters>\n\n<suggestion for how to fix>"`
- `tags: ["blocker"]` or `tags: ["warning"]`

Then add subtasks that break the fix into concrete steps. Use `kanban` with:
- `op: "add subtask"`
- `task_id: "<task-id>"`
- `title: "<specific fix step>"`

Every card MUST include a subtask for verifying the fix (running tests, re-checking the logic, etc.).

#### Ordering

Blockers come first. Within each severity, order cards by the layer they were found in (design issues before style issues). Set dependencies between cards when one fix must land before another makes sense.

### 6. Summarize

End with a summary:

- One-sentence overall assessment
- Count of findings by severity (e.g., "1 blocker, 3 warnings, 5 nits")
- List of kanban cards created with their IDs and titles
- Verdict: **approve**, **request changes**, or **comment only**
  - **Approve**: no blockers, warnings are minor or acceptable
  - **Request changes**: blockers exist, or warnings are serious enough to address first
  - **Comment only**: you lack enough context to approve or reject — findings are informational
- Any nits that were not captured as cards, listed inline for reference

## Language-Specific Guidelines

After completing the universal layers above, apply language-specific review criteria based on the project being reviewed. These supplement — not replace — the universal layers.

Consult the matching resource file bundled with this skill:

| Language | Resource | Authority |
|----------|----------|-----------|
| Rust | `RUST_REVIEW.md` | dtolnay (serde, thiserror, anyhow) |
| Dart / Flutter | `DART_FLUTTER_REVIEW.md` | Remi Rousselet (Riverpod, freezed) |
| Python | `PYTHON_REVIEW.md` | Hynek Schlawack (attrs, structlog) |
| JavaScript / TypeScript | `JS_TS_REVIEW.md` | Sindre Sorhus (xo, got, execa) |

If the project uses multiple languages, apply all relevant sections. If none of the above match, rely on the universal layers alone.

Language-specific findings follow the same severity levels (blocker, warning, nit) and are captured as kanban cards using the same process described above.

## Scope Exclusions

- **Skip gitignored files.** Do not review any file matched by `.gitignore`. These are build artifacts, dependencies, and generated output — not authored code.
- **Skip dot-directories.** Do not review files under directories starting with `.` (e.g., `.git/`, `.vscode/`, `.skills/`, `.config/`) unless the user explicitly asks you to include them. Dot-directories contain tooling configuration, not project source code.

## Guidelines

- **Facts over opinions.** Technical arguments and data beat personal preference. If you can't explain why something is wrong beyond "I'd do it differently," it's a nit at best.
- **Review the change, not the whole file.** Flag pre-existing issues only if the change makes them worse or if they directly interact with the changed code.
- **Don't block on style.** If the project has a style guide or formatter, defer to it. If there is no convention, accept the author's style. Never block a change on purely cosmetic grounds.
- **Size your review to the change.** A one-line fix doesn't need an architecture essay. A large refactor deserves deep examination. Match effort to scope.
- **Be specific.** "This function is confusing" is not actionable. "This function mixes validation and persistence — splitting them would make each testable independently" is actionable.
- **One concern per finding.** Don't bundle unrelated issues into a single comment.
- **Assume nothing.** AI-generated code can look polished but be subtly wrong. Verify logic — don't trust that professional-looking code is correct.
