---
name: review
profiles:
  - kanban
description: Code review workflow. Use this skill whenever the user says "review", "code review", "review this PR", "review my changes", or otherwise wants a code review. Reviews produce verbose output — automatically delegates to a reviewer subagent.
context: fork
agent: reviewer
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool (for symbol lookup, callgraph, and blast-radius during review) and the `kanban` MCP tool (to drive tasks through the review column and capture follow-up findings). 
metadata:
  author: "swissarmyhammer"
  version: "{{version}}"
---


# Code Review

Perform a structured code review. Findings land as a GFM checklist on a kanban task — attached to the work, not piling up as new tasks.

Here is what the user provided: 
$ARGUMENTS

## Guidelines

{% include "_partials/coding-standards" %}
{% include "_partials/review-column" %}
{% include "_partials/architecture-awareness" %}

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

### 3. Get the changes

`git` `op: "get changes"` to scope the diff.

**Task-mode** — read the task first:

```json
{"op": "get task", "id": "<id>"}
```

Use any range hint in the description (commit range, branch, PR ref) to scope. No hint → `{"op": "get changes"}` auto-detects.

**Range-mode**:

| User says | Call |
|-----------|------|
| `/review` (review column empty) | `{"op": "get changes"}` — auto-detects |
| `/review the last 4 commits` | `{"op": "get changes", "range": "HEAD~4..HEAD"}` |
| `/review since abc123` | `{"op": "get changes", "range": "abc123..HEAD"}` |
| `/review abc123..def456` | `{"op": "get changes", "range": "abc123..def456"}` |
| `/review feature-branch` | `{"op": "get changes", "branch": "feature-branch"}` |

Read every changed file in full — diffs alone lack context. Understand the **purpose** of the change before reviewing (PR description, commit messages, kanban task body).

When a `range` was used (explicit or auto-detected), use `get diff` with `file@<ref>` for semantic diffs:

```json
{"op": "get diff", "left": "src/main.rs@HEAD~4", "right": "src/main.rs"}
```

For every changed file: `{"op": "get blastradius", "file_path": "<file>"}` and `get callgraph` (inbound) on changed symbols. Diff shows what changed; blast radius shows what it *affects* — sizes the change for Layer 1, finds untouched callers the change may have quietly broken for Layer 3. An **empty inbound callgraph** on an added/changed symbol that isn't an entry point, exported API, or test is the dead-code signal for Layer 2.

Run `{"op": "find duplicates", "file_path": "<file>"}` on the changed files — the verbatim/near-verbatim duplication signal for Layer 2.

### 4. Layered examination

Don't skip layers — each catches different problems.

**Layer 1: Design & Architecture** — Does it fit? Appropriate abstractions? Over-engineering? Right codebase for this?

**Layer 2: Reuse, Dead Code & Data-Driven Design** — the highest-leverage layer for machine-written code, which trends toward duplication and hardcoding. Findings here are **blockers**.

- **Dead code (blocker)** — any added or changed symbol with an empty inbound callgraph that is not an entry point, exported public API, or test is dead. Also flag orphaned modules never wired into production, unreachable branches, commented-out code, and tests that exercise only a dead path. Delete it; don't ship it.
- **Duplication (blocker for verbatim/near-verbatim)** — copies drift out of sync and inflate the surface area. Extract a shared function and parameterize the difference. Two blocks that differ only by a value are one function with an argument.
- **Hardcoding → data** — be data-driven. A `match`/`if`-chain over a known set whose arms differ only in constants is a table, not control flow. Repeated literals are a named constant or config entry. Variation belongs in data (tables, maps, config, declarative specs) interpreted by a single code path — not in parallel code paths a human must keep in lockstep.
- **Calibration** — warranted generalization removes *existing* duplication or serves a *real* variation axis. Rule of three: two occurrences is coincidence, three is a pattern. No second caller → no parameter. The right abstraction beats three copies; the wrong abstraction is worse than five. Speculative abstraction with no real consumer is over-engineering — flag it under Layer 1, not here.

**Layer 3: Functionality & Correctness** — Does it do what the author intended? Good for users? Edge cases (empty, null, boundary, error)? Off-by-one, wrong booleans, missing early returns? Concurrency (races, deadlocks, shared mutable state)?

**Layer 4: Tests** — Tests for new/changed behavior? Verify behavior, not implementation? Would they fail if the code were broken? Edge cases covered? Mocks only at system boundaries?

**Layer 5: Security** — Input validated? Injection (SQL, command, XSS, template)? Secrets safe? Auth checks? Error messages safe?

**Layer 6: Naming, Clarity, Simplicity** — Descriptive without being verbose? Understandable without explanation? Comments explain "why"? Stale comments or TODOs?

**Layer 7: Performance** (when relevant) — O(n²)+ on large data? Unnecessary allocations in hot paths? N+1 queries? Resource cleanup in all paths?

### 5. Review every line

If code is hard to understand, that's itself a finding.

### 6. Apply language-specific guidelines

| Language | File |
|----------|------|
| Rust | [RUST_REVIEW.md](./references/RUST_REVIEW.md) |
| Dart / Flutter | [DART_FLUTTER_REVIEW.md](./references/DART_FLUTTER_REVIEW.md) |
| Python | [PYTHON_REVIEW.md](./references/PYTHON_REVIEW.md) |
| JavaScript / TypeScript | [JS_TS_REVIEW.md](./references/JS_TS_REVIEW.md) |

Multi-language project → apply all relevant sections. Same severity levels.

### 7. Architecture review

If `ARCHITECTURE.md` exists at the project root, add an alignment layer:

- **Follows documented architecture?** New code in the right module/layer/component. Flag changes that bypass boundaries (handler calling DB directly when architecture specifies a service layer).
- **New components placed correctly?** New files/modules/crates fit the structure?
- **Requires an architecture update?** Intentional divergence or extension (new module, dependency direction, service) → finding recommending `ARCHITECTURE.md` update.

Architecture findings use the same severity levels — boundary violation = **warning**; undocumented structural addition = **nit** unless it contradicts an explicit constraint (then **blocker**).

No `ARCHITECTURE.md` → skip.

### 8. Format findings as a dated GFM checklist

Single dated section, organized by severity (current local date/time):

```markdown
## Review Findings (2026-04-11 13:08)

### Blockers
- [ ] `path/to/file.rs:42` — What's wrong. Why it matters. Suggested fix.

### Warnings
- [ ] `path/to/file.rs:10` — What's wrong and suggested fix.

### Nits
- [ ] `path/to/file.rs:88` — Minor issue.
```

| Severity | Meaning |
|----------|---------|
| **blocker** | Correctness bug, security vuln, data loss risk, dead code, or verbatim/near-verbatim duplication |
| **warning** | Design problem, missing test, performance concern |
| **nit** | Style preference, minor improvement |

Each finding: **where** (file:line), **what**, **why** (skip for nits), **suggestion** when non-obvious. Omit empty severity subsections.

**One concern per checklist item.** Don't bundle. Three components with the same problem = three items.

### 9. Apply findings

Never create one kanban task per finding. Findings = checklist items on a host task — the task being reviewed (task-mode) or a single tracking task (range-mode).

#### Task-mode

1. Re-read the target task (already have it from step 3): `{"op": "get task", "id": "<id>"}`.

2. If not in `review`, move it there first (covers manual `/review <id>` on a task still in `todo`/`doing`):

   ```json
   {"op": "move task", "id": "<id>", "column": "review"}
   ```

   No-op when it came from `implement` already in `review`.

3. Parse the description for prior `## Review Findings (...)` sections; note whether every `- [ ]` has been flipped to `- [x]`.

4. Outcome:
   - **Zero new findings AND every prior item checked** → move to terminal column:

     ```json
     {"op": "move task", "id": "<id>", "column": "done"}
     ```

     Leave description history intact.

   - **New findings OR any prior item still unchecked** → append a new dated `## Review Findings (YYYY-MM-DD HH:MM)` section, write it back:

     ```json
     {"op": "update task", "id": "<id>", "description": "<existing + blank line + new section>"}
     ```

     Preserve existing description verbatim — never edit or delete prior sections. Task stays in `review`.

#### Range-mode

1. Fresh review with **zero findings** → "clean, nothing to track", exit. Do NOT create a tracking task.

2. Otherwise create a tracking task in `review`. First ensure the `#review` tag exists:

   ```json
   {"op": "list tags"}
   ```

   Missing → `{"op": "add tag", "id": "review", "name": "Review", "color": "9900cc", "description": "Ad-hoc range review tracking"}`.

3. Create directly in `review`:

   ```json
   {"op": "add task", "title": "Review of <scope>", "description": "Scope: <range or branch>\n\n## Review Findings (YYYY-MM-DD HH:MM)\n\n### Blockers\n- [ ] ...\n\n### Warnings\n- [ ] ...", "column": "review"}
   ```

4. Tag it: `{"op": "tag task", "id": "<new-id>", "tag": "review"}`.

   A subsequent `/review <tracking-id>` follows task-mode and moves it to terminal when all items are checked and a fresh review is clean.

### 10. Summarize

- **Mode**: task-mode (with id) or range-mode (with scope)
- **Scope reviewed**: effective range or branch
- **Counts**: by severity ("1 blocker, 3 warnings, 5 nits" or "clean")
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
2. `get task` → read body, scope the diff.
3. `get changes` auto-detect, read every changed file, apply seven layers (+ RUST_REVIEW.md for Rust).
4. Zero new findings, all prior items now `- [x]`.
5. Move to `done`.

The column move is the verdict — no findings appended, history preserved.

**Range-mode with findings:** `/review the last 4 commits`.

1. Ensure review column.
2. `review` empty → range-mode. `get changes range: "HEAD~4..HEAD"`.
3. For each file, `get diff left: "src/server.rs@HEAD~4" right: "src/server.rs"`.
4. Layered review → 1 blocker (missing auth check), 2 nits.
5. Ensure `#review` tag.
6. Create tracking task in `review` with the dated findings checklist.
7. Tag it `review`.

Subsequent `/review <new-id>` follows task-mode — moves to `done` once items are checked and a re-review is clean.

## Rules

- **Facts over opinions.** Technical arguments beat personal preference.
- **Review the change, not the whole file.** Flag pre-existing issues only if the change makes them worse.
- **Don't block on style.** Defer to formatters; accept the author's style if no convention exists.
- **Specific and actionable.** "This is confusing" isn't enough — say what's confusing and what to do.
- **One concern per checklist item.** Don't bundle.
- **No per-finding tasks.** Findings = checklist items on the source task (task-mode) or a single tracking task (range-mode). The retired `review-finding` tag — don't create or reuse it.
- **Preserve history on re-run.** Always append new dated sections. Never edit or delete prior ones; never flip checkboxes yourself — the user (or the implementer picking up the task) owns the marks.
- **Skip gitignored files and dot-directories** (`.git/`, `.vscode/`, `.skills/`) unless explicitly asked.
