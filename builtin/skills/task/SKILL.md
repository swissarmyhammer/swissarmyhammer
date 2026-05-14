---
name: task
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

Create a single, well-researched kanban task from an idea, request, or bug report.

{% if arguments %}
## User Request

> {{arguments}}
{% endif %}

## Process

### 1. Understand the idea

{% if arguments %}Start from the user request above.{% endif %} If anything is ambiguous or underspecified, use the `question` tool to ask clarifying questions before proceeding. A great task requires clear understanding — don't guess.

### 2. Research the codebase

Use `code_context` as the primary research tool:

- **Find symbols** — `op: "search symbol"` with domain keywords, `op: "get symbol"` for implementations
- **Map blast radius** — `op: "get blastradius"` on files you expect the work to touch. This reveals callers, downstream consumers, tests, and transitive dependencies.
- **Trace call chains** — `op: "get callgraph"` with `direction: "inbound"` and `"outbound"` to understand execution flow
- **Fall back to text search** — Glob, Grep, Read for string literals, config files, or patterns not in the index

Thorough research is always required. The tools you use may differ — a bug fix may focus on blast radius while a feature requires broader symbol exploration — but never skip research because something appears simple.

### 3. Create the task

Create the task on the kanban board using `kanban` with `op: "add task"`. The task must meet the task standards included above — What, Acceptance Criteria, and Tests sections are mandatory.

If the research reveals the work is too large for a single task (exceeds sizing limits), tell the user and suggest they use `/plan` instead.

### 4. Present the result

Show the user the task you created — title, description, and any tags applied.

## Examples

### Example 1: tracking a bug report as a single well-formed task

User says: "track this bug — parser panics on empty input"

Actions:
1. Research with `{"op": "search symbol", "query": "parse"}` to locate `Parser::parse_input` in `src/parser.rs`, then `{"op": "get symbol", "query": "Parser::parse_input"}` to confirm the panic path (an `unwrap()` on an empty slice).
2. Map `{"op": "get blastradius", "file_path": "src/parser.rs", "max_hops": 2}` to find tests (`tests/parser.rs`) and callers (three handlers) that may need updating.
3. Create the task via `kanban` `op: "add task"` with What / Acceptance Criteria / Tests:

   ```json
   {
     "op": "add task",
     "title": "Parser panics on empty input",
     "description": "## What\nParser::parse_input calls `slice[0]` without checking length, panicking on `\"\"`. Return a `ParseError::Empty` instead.\n\n## Acceptance Criteria\n- [ ] `Parser::parse_input(\"\")` returns `Err(ParseError::Empty)`, not a panic\n- [ ] All three call sites handle the new error variant\n\n## Tests\n- [ ] Unit test in `tests/parser.rs` for empty-input case\n- [ ] `cargo test parser::empty` passes"
   }
   ```

4. Present the created task id, title, and description to the user.

Result: One well-researched task on the board with concrete file paths, acceptance criteria, and a runnable test command.

### Example 2: ambiguous request — ask before writing

User says: "add a task for the login thing we discussed"

Actions:
1. The request is too vague — use `{"op": "ask question", "question": "Which login concern should I track? The JWT-refresh retry logic, the password reset form, or the rate limiter on POST /api/login?"}` before proceeding.
2. After the user clarifies ("the rate limiter"), research the relevant code, then create a single well-formed task.

Result: A task the implementer can act on without needing to re-derive the intent — the clarification happens at task creation, not at implementation time.

## Constraints

- **One task per invocation.** If the user describes multiple pieces of work, create one task for the most important item and suggest `/plan` for the rest.
- **Research before writing.** Don't guess at file paths, function names, or test locations. Look them up.
- **Ask, don't assume.** If the user's request is vague or could be interpreted multiple ways, use the `question` tool to clarify before creating the task.
- **Task quality is non-negotiable.** Every task must have What, Acceptance Criteria, and Tests. A task without these is not valid.
- **Use the kanban board.** Do NOT use TodoWrite, TaskCreate, or any other task tracking. The kanban board is the single source of truth.
