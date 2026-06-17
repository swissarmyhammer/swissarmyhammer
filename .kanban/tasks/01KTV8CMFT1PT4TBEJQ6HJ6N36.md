---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvaaxyp8h9kwzaqnghq9h0wc
  text: |-
    Implemented via TDD.

    RED: added 3 dispatch tests (dispatch_list_tasks_project_param_scopes_to_project, _intersects_with_filter, _unknown_project_returns_empty) + 1 schema test. Before the fix all failed because `project` was silently ignored: scopes test got count 2 (whole board) vs expected 1; intersect got 2 vs 1; unknown got 1 vs 0.

    GREEN (after fix):
    - crates/swissarmyhammer-kanban/src/dispatch.rs Verb::List arm: read op.get_string("project"); fold into ListTasks filter — alone -> `$<project>`, with explicit filter -> `<filter> && $<project>`. No project resolution added in dispatch (slug registry inside ListTasks::execute handles id/name-slug case-insensitive).
    - crates/swissarmyhammer-kanban/src/task/list.rs: added optional `project: Option<String>` field with doc comment so the FULL schema documents it. Being Option<T>, it stays out of x-op-signatures (wire signature unchanged).
    - crates/swissarmyhammer-kanban/src/schema.rs: added test_project_param_absent_from_signatures_but_in_full_schema asserting project documented in x-operation-schemas and NOT in x-op-signatures required list.

    Verification:
    - cargo nextest run -p swissarmyhammer-kanban -> 1499 passed, 0 failed.
    - cargo clippy -p swissarmyhammer-kanban -- -D warnings -> clean, 0 warnings.
    - 4 new tests pass; confirmed RED->GREEN on the project-param dispatch tests.
  timestamp: 2026-06-17T08:24:57.800532+00:00
- actor: wballard
  id: 01kvabf2384pjd1bcn51qb4r4j
  text: 'Moved to done by /finish orchestrator. All six acceptance criteria independently verified MET (project-only scoping, project+filter AND, unknown-project→empty, no-project path unchanged, x-op-signatures unchanged + full schema documents project, clippy clean). Tests: cargo nextest -p swissarmyhammer-kanban 1499/1499 passed; the 3 new dispatch tests + schema test verified RED→GREEN. The single review-engine finding (list.rs:45 — project lacks a with_project() builder, unlike other optional ListTasks fields) is a non-blocking API-consistency nit, not a contract defect: project is deliberately coerced at dispatch via op.get_string + with_filter and is never set on the struct by a builder in production (only via Deserialize). Not addressed — outside this task''s scope. Could be a small follow-up if builder symmetry is desired.'
  timestamp: 2026-06-17T08:34:18.344328+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb980
title: 'Fix silent ignore: list tasks project param returns whole board — coerce to $<project> filter'
---
## What
The `list tasks` dispatch arm (`execute_task_query_operation`, `Verb::List`, in `crates/swissarmyhammer-kanban/src/dispatch.rs`) reads only `column`, `filter`, `page`, and `page_size`. A `project` param is silently ignored and the whole board is returned — the agent believes it got a project-scoped listing and acts on the wrong task set.

Decision: HONOR the param (coerce to the filter DSL), do not error. Rationale, based on how the rest of the op surface treats extra params: dispatch never validates a param allowlist anywhere — unknown/extra params are ignored by every op — and the system's documented contract is forgiving input with aliases and inference (`assignee` → `assignees` single-string fallback in `resolve_assignees`; short ids/prefixes → ULIDs in `resolve_task_ref`). Erroring on `project` alone would be inconsistent with that contract, and `project` has an exact DSL equivalent, so inference is the consistent forgiving behavior: `project: "myproj"` ≡ filter `$myproj`.

Implementation:
- [x] In the `Verb::List` arm: read `op.get_string("project")`; when present, fold it into the `ListTasks` filter — alone → `$<project>`; together with an explicit `filter` → `<filter> && $<project>` (the DSL supports `&&` conjunction, see the `"#bug && @alice"` test in `crates/swissarmyhammer-kanban/src/task/list.rs`).
- [x] Project ref by id or name-slug, case-insensitive, comes free from `EntitySlugRegistry` inside `ListTasks::execute` — no extra resolution in dispatch.
- [x] A `project` value matching no project yields an empty listing (normal `$` filter semantics), not an error.
- [x] Document the optional `project` param in the `list tasks` entry of the full schema (`generate_kanban_mcp_schema_full` / `x-operation-schemas`); the wire `x-op-signatures` required list is unchanged (param is optional).

## Acceptance Criteria
- [x] `{"op": "list tasks", "project": "<id>"}` returns only that project's tasks (regression: previously returned every task on the board).
- [x] `project` combined with `filter` applies both (AND semantics).
- [x] `project` naming a nonexistent project returns an empty list, not the whole board.
- [x] `list tasks` without `project` is byte-for-byte unchanged.
- [x] Wire `x-op-signatures` for `list tasks` unchanged; full schema documents `project`.
- [x] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean.

## Tests
- [x] Dispatch tests in `crates/swissarmyhammer-kanban/src/dispatch.rs` `#[cfg(test)]` (TempDir board pattern): board with a project (`AddProject`) and tasks in/out of it — `project` param returns only the in-project task (fails before the fix: count equals the whole board); `project` + `filter` (e.g. a `#tag`) intersect; unknown `project` returns count 0.
- [x] `cargo nextest run -p swissarmyhammer-kanban` — green.

## Workflow
- Use `/tdd` — write the failing project-param dispatch test first, then implement the filter coercion.

## Review Findings (2026-06-17 03:26)

Engine: 0 blockers, 1 warning, 0 nits (1/15 review tasks failed — coverage incomplete; behavioral + schema criteria independently verified by reviewer).

All six acceptance criteria verified MET:
- Regression fix, project+filter AND, unknown-project-empty: covered by `dispatch_list_tasks_project_param_scopes_to_project`, `dispatch_list_tasks_project_param_intersects_with_filter`, `dispatch_list_tasks_unknown_project_returns_empty` — all green (1499/1499).
- No-project path unchanged: dispatch `(None, None) => None` leaves filter untouched.
- x-op-signatures unchanged (`[]`) + full schema documents `project`: `test_project_param_absent_from_signatures_but_in_full_schema` green; live MCP schema confirms.
- `cargo clippy -p swissarmyhammer-kanban -- -D warnings` clean (exit 0).
- Filter coercion is correct (`format!("{filter} && ${project}")`); `$`-atom resolution is delegated to the slug registry inside `ListTasks::execute`, consistent with `@`/`^` predicates.

### Warnings (reviewer assessment: non-blocking style nit, not a contract defect)
- [ ] `crates/swissarmyhammer-kanban/src/task/list.rs:45` — `project` is the only optional `ListTasks` field without a `with_project()` builder method (`column`/`filter`/`page`/`page_size`/`detail` all have one). Factually accurate, but immaterial to this task: by design the coercion happens at dispatch (`op.get_string("project")` → folded into `with_filter`), so production never sets `project` on the struct via a builder — it arrives only via `Deserialize`. Optional follow-up for API consistency; does not violate any acceptance criterion.