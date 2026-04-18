---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffda80
title: Match filter entity predicates ($project, @actor, ^task) on id OR slug with a single canonical Rust slugify
---
## What

Filter predicates `$project-slug`, `@user-slug`, `^task-slug` currently match only on the **stored value** of the referenced field (e.g. a task's `project` field — which is the project's id, typically the filename stem). The autocomplete offers the **slug of the current `name`**, computed by a TypeScript `slugify` in `kanban-app/ui/src/lib/slugify.ts`. Those two strings diverge whenever a project/actor/task has been renamed after creation — its id stays but its name-slug changes — so picks from autocomplete produce filters that match nothing.

### Concrete reproducer

- Project file on disk: `.kanban/projects/task-card-fields.yaml` → id = `task-card-fields`.
- Current `name` in that file: `Task card & field polish`.
- Frontend slugifies the name → autocomplete shows `$task-card-field-polish`.
- User picks it. Filter becomes `$task-card-field-polish`.
- Backend `has_project` (swissarmyhammer-kanban/src/task_helpers.rs:538) compares the filter string to the task's `project` field. Tasks in that project have `project: task-card-fields` (the id). No match. Empty board.

Same shape for:
- `@actor` — actor id on disk vs. slug of display name (has_assignee at task_helpers.rs:522).
- `^task` — task id vs. slug of title (has_ref at task_helpers.rs:529).

### Architectural fix

**One canonical slug function in Rust.** Frontend's `slugify.ts` becomes a one-line mirror of the Rust logic — or, better, the Rust logic ships as WASM and the TS calls it. Either way, the frontend and backend produce BYTE-IDENTICAL slugs for every input, forever.

**Filter engine matches on id OR slug** for each entity predicate:
- `$project-value` matches a task whose `project` field equals `value` (current behavior) OR whose project's name slugifies to `value`.
- `@actor-value` matches a task whose `assignees` contains `value` (current) OR contains an actor id whose `name` slugifies to `value`.
- `^task-value` matches a task whose `id == value` OR `depends_on` contains `value` (current) OR whose title/id slug equals `value`.

The `FilterContext` trait in `swissarmyhammer-filter-expr/src/eval.rs` needs access to the project/actor registry to compute this. The `TaskFilterAdapter` in `swissarmyhammer-kanban/src/task_helpers.rs` is where the enrichment happens.

## Files to touch

### Rust (backend)

- **Create** `swissarmyhammer-slug/` (or put in an existing crate like `swissarmyhammer-common`) — one public function `pub fn slug(s: &str) -> String` matching the TS rules:
  - lowercase
  - replace runs of non-`[a-z0-9]` with a single `-`
  - strip leading/trailing `-`
  - idempotent
- `swissarmyhammer-kanban/src/task_helpers.rs` — update the `TaskFilterAdapter` impl of `has_project`, `has_assignee`, `has_ref` to consult the project/actor/task registries and match on id OR slug(name).
  - Currently adapter only holds `entity: &Entity`. Extend it with references to the enriched project/actor/task maps so the lookups can happen.
- `swissarmyhammer-filter-expr` — if any test fixtures use slugs, update to exercise the id-or-slug contract.

### TypeScript (frontend)

- `kanban-app/ui/src/lib/slugify.ts` — either:
  - (a) keep but add a Rust parity test that generates a corpus of strings, slugifies in both implementations, and asserts byte equality; or
  - (b) replace with a WASM binding to the Rust function.
  Option (a) is lower risk for this pass; (b) is the long-term right answer.
- `kanban-app/ui/src/hooks/use-mention-extensions.ts` and `cm-mention-autocomplete.ts` — autocomplete already returns `slug` for display; verify it calls the same slugify so frontend display and backend filter matching align.

## Acceptance Criteria

- [x] A single Rust `slug(s: &str) -> String` function exists in one crate, with unit tests covering: empty, punctuation, uppercase, leading/trailing non-alphanumeric, runs of non-alphanumeric, idempotent second application, unicode punctuation.
- [x] Frontend `slugify.ts` either calls the Rust function (via WASM) OR a parity test runs both over a 100+ item corpus and asserts every output matches.
- [x] `has_project(value)` returns true when the task's project id === value OR the task's project name slugifies to value.
- [x] `has_assignee(value)` returns true when the task's assignee list contains an id === value OR an actor whose name slugifies to value.
- [x] `has_ref(value)` returns true when the task id === value OR depends_on contains value OR a referenced task's title slugifies to value.
- [x] The concrete reproducer above passes: `$task-card-field-polish` filter returns the tasks actually in project `task-card-fields` (since its name "Task card & field polish" slugifies to `task-card-field-polish`).

## Tests

- [x] `cargo test -p <slug-crate>` — slug function unit tests.
- [x] `cargo test -p swissarmyhammer-kanban --lib task_helpers` — TaskFilterAdapter tests for id-or-slug matching on $project, @actor, ^task.
- [x] Parity test asserting TS and Rust slugify produce identical output on a shared corpus.
- [x] Frontend: the existing mention-autocomplete tests still pass.
- [x] Manual: run `pnpm tauri dev`, filter by `$task-card-field-polish`, confirm tasks appear.

## Workflow

- Use `/tdd` — write the `slug` crate tests + the parity corpus test + the filter-adapter id-or-slug tests before changing eval behavior.
- Do NOT change the filter DSL grammar. The values after `$`/`@`/`^` still parse as identifier strings; the change is purely in matching semantics.

#bug #kanban #filter #backend #frontend