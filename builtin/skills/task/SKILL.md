---
name: task
profiles:
  - kanban
description: Create a single, well-researched kanban task. Use when the user wants to add a task, track an idea, or capture work without entering full plan mode.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for researching symbols and impact before writing the task, and the `kanban` MCP tool to persist the task on the board. Both are provided by the swissarmyhammer `sah` MCP server; will not function on a harness that does not expose them.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

{% include "_partials/coding-standards" %}
{% include "_partials/task-standards" %}

# Task

Create one well-researched kanban task from an idea, request, or bug report.

{% if arguments %}
## User Request

> {{arguments}}
{% endif %}

## Process

### 1. Understand

{% if arguments %}Start from the request above.{% endif %} If ambiguous or underspecified, use the `question` tool to clarify before proceeding. Don't guess.

### 2. Research

Use `code_context` as the primary tool:

- **Symbols** — `search symbol` with domain keywords; `get symbol` for implementations
- **Blast radius** — `get blastradius` on expected target files to reveal callers, downstream consumers, tests, transitive deps
- **Call chains** — `get callgraph` with `direction: "inbound"` / `"outbound"`
- **Fallback** — Glob/Grep/Read for string literals, config files, patterns not in the index

Thorough research is always required. The mix varies (bugs focus on blast radius; features need broader exploration) but never skip because something looks simple.

### 3. Create

`kanban` `op: "add task"`. Must meet the task standards above — What, Acceptance Criteria, Tests are mandatory.

If research shows the work is too large for one task (exceeds sizing limits), tell the user and suggest `/plan`.

### 4. Present

Show the user the task — title, description, tags.

## Examples

**Bug report as a task:** User says "track this bug — parser panics on empty input".

1. `{"op": "search symbol", "query": "parse"}` → `Parser::parse_input` in `src/parser.rs`. `get symbol` confirms an `unwrap()` on an empty slice.
2. `{"op": "get blastradius", "file_path": "src/parser.rs", "max_hops": 2}` → `tests/parser.rs` + three callers.
3. Create the task:

   ```json
   {
     "op": "add task",
     "title": "Parser panics on empty input",
     "description": "## What\nParser::parse_input calls `slice[0]` without checking length, panicking on `\"\"`. Return a `ParseError::Empty` instead.\n\n## Acceptance Criteria\n- [ ] `Parser::parse_input(\"\")` returns `Err(ParseError::Empty)`, not a panic\n- [ ] All three call sites handle the new error variant\n\n## Tests\n- [ ] Unit test in `tests/parser.rs` for empty-input case\n- [ ] `cargo test parser::empty` passes"
   }
   ```

4. Present the task id, title, description.

**Ambiguous request — ask first:** User says "add a task for the login thing we discussed". Use `{"op": "ask question", "question": "Which login concern — JWT-refresh retry, password reset form, or rate limiter on POST /api/login?"}` before proceeding. After the answer, research and create.

## Constraints

- **One task per invocation.** Multiple items → pick the most important, suggest `/plan` for the rest.
- **Research before writing.** No guessing at paths, names, test locations.
- **Ask, don't assume.** Vague requests get the `question` tool.
- **Task quality is non-negotiable** — What + Acceptance Criteria + Tests.
- **Kanban only** — no TodoWrite/TaskCreate.
