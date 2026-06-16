---
assignees:
- claude-code
position_column: todo
position_ordinal: 9b80
title: 'Fix silent ignore: list tasks project param returns whole board — coerce to $<project> filter'
---
## What
The `list tasks` dispatch arm (`execute_task_query_operation`, `Verb::List`, in `crates/swissarmyhammer-kanban/src/dispatch.rs`) reads only `column`, `filter`, `page`, and `page_size`. A `project` param is silently ignored and the whole board is returned — the agent believes it got a project-scoped listing and acts on the wrong task set.

Decision: HONOR the param (coerce to the filter DSL), do not error. Rationale, based on how the rest of the op surface treats extra params: dispatch never validates a param allowlist anywhere — unknown/extra params are ignored by every op — and the system's documented contract is forgiving input with aliases and inference (`assignee` → `assignees` single-string fallback in `resolve_assignees`; short ids/prefixes → ULIDs in `resolve_task_ref`). Erroring on `project` alone would be inconsistent with that contract, and `project` has an exact DSL equivalent, so inference is the consistent forgiving behavior: `project: "myproj"` ≡ filter `$myproj`.

Implementation:
- [ ] In the `Verb::List` arm: read `op.get_string("project")`; when present, fold it into the `ListTasks` filter — alone → `$<project>`; together with an explicit `filter` → `<filter> && $<project>` (the DSL supports `&&` conjunction, see the `"#bug && @alice"` test in `crates/swissarmyhammer-kanban/src/task/list.rs`).
- [ ] Project ref by id or name-slug, case-insensitive, comes free from `EntitySlugRegistry` inside `ListTasks::execute` — no extra resolution in dispatch.
- [ ] A `project` value matching no project yields an empty listing (normal `$` filter semantics), not an error.
- [ ] Document the optional `project` param in the `list tasks` entry of the full schema (`generate_kanban_mcp_schema_full` / `x-operation-schemas`); the wire `x-op-signatures` required list is unchanged (param is optional).

## Acceptance Criteria
- [ ] `{"op": "list tasks", "project": "<id>"}` returns only that project's tasks (regression: previously returned every task on the board).
- [ ] `project` combined with `filter` applies both (AND semantics).
- [ ] `project` naming a nonexistent project returns an empty list, not the whole board.
- [ ] `list tasks` without `project` is byte-for-byte unchanged.
- [ ] Wire `x-op-signatures` for `list tasks` unchanged; full schema documents `project`.
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [ ] Dispatch tests in `crates/swissarmyhammer-kanban/src/dispatch.rs` `#[cfg(test)]` (TempDir board pattern): board with a project (`AddProject`) and tasks in/out of it — `project` param returns only the in-project task (fails before the fix: count equals the whole board); `project` + `filter` (e.g. a `#tag`) intersect; unknown `project` returns count 0.
- [ ] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the failing project-param dispatch test first, then implement the filter coercion.