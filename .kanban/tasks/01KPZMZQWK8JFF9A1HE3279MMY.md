---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9780
title: Move resolveFocusedColumnId out of the React UI into Rust UIState
---
## What

PR #40 review comment from @wballard on `kanban-app/ui/src/components/board-view.tsx:609`:

> again - no way this belongs in the UI -- this should be UIState - headless rust testable without the gui

Ref: https://github.com/swissarmyhammer/swissarmyhammer/pull/40#discussion_r3137384797

`resolveFocusedColumnId` lives in `kanban-app/ui/src/components/board-view.tsx` (TypeScript / React). It walks the focus scope chain — `column:{id}` moniker resolves directly; `task:{id}` resolves via `taskMap` to the task's home column; anything else returns `null` so the backend can fall back to the lowest-order column.

The reviewer's architectural position: this resolution is business logic, not presentation. It belongs in Rust UIState, exercisable from headless tests (no jsdom, no React rendering, no mock stores).

### Files to modify

- `swissarmyhammer-commands/src/ui_state.rs` (or `swissarmyhammer-kanban/src/`) — add a method/free-fn like `UIState::resolve_focused_column(scope_chain: &[String], task_to_column: &HashMap<TaskId, ColumnId>) -> Option<ColumnId>` — or whatever the appropriate input shape is based on how `taskMap` is currently sourced.
- `kanban-app/src/commands.rs` (or wherever `AddEntity` dispatches) — if the column-fallback decision currently rides on the frontend-passed `column_id`, the Rust side may need to consume the scope chain directly instead of trusting a frontend-resolved value. This changes the backend contract.
- `kanban-app/ui/src/components/board-view.tsx` — delete `resolveFocusedColumnId`. Instead of computing a column id in React, pass the scope chain through (already present in dispatch) and let the backend resolve.
- `kanban-app/ui/src/components/board-view.tsx` callers of `resolveFocusedColumnId` — adjust to the new contract.

### Investigation needed

- Is `taskMap` (the task id → column id map) something UIState already has, or does it come from a store query? If it comes from a store query, the Rust resolver can use whatever primary key the backend already has.
- Does the React codebase use `resolveFocusedColumnId` outside `board-view.tsx`? A grep should make this clear before moving.
- The scope chain is already part of dispatch payloads; the backend already sees it. This task is really about *using* it for the column resolution instead of pre-resolving in React.

## Acceptance Criteria

- [x] `kanban-app/ui/src/components/board-view.tsx` no longer contains a `resolveFocusedColumnId` function.
- [x] The equivalent resolver lives in Rust and is exercised by a headless Rust test asserting all three branches (column moniker direct, task moniker via map, fallback to `None` / lowest-order column).
- [x] `AddEntity` (and any other caller that needs the focused column) consumes the scope chain and resolves internally.
- [x] No regression: adding a new task on a board view still places it in the expected column when (a) a column is focused, (b) a task is focused, (c) nothing is focused.

## Tests

- [x] Add `swissarmyhammer-kanban/tests/resolve_focused_column.rs` (new) — asserts:
  - scope chain with `column:<id>` → returns that id.
  - scope chain with `task:<tid>` and a task map containing `tid → <cid>` → returns `<cid>`.
  - scope chain with something else → returns `None`.
  - scope chain with a `task:*` that has no entry in the map → returns `None` (not a random default).
- [x] Update `board-view.tsx` tests — remove tests for the deleted React function, add an integration test that proves dispatch flows through without pre-resolving in the UI.
- [x] Manual or fake-dispatch integration: create new task with focus on `column:todo` ⇒ lands in todo. Create new task with focus on `task:<x in doing>` ⇒ lands in doing. Create new task with no focus ⇒ lands in the default (lowest-order) column.

## Workflow

Use `/tdd`. Write the Rust resolver tests first (all three branches), implement the resolver, then delete the React function and adjust the dispatch call sites. #commands #refactor #architecture #uistate #frontend

## Implementation Notes (2026-04-23)

**Resolver placement.** Put in `swissarmyhammer-kanban/src/focus.rs` as the task suggested — a pure, free fn `resolve_focused_column(scope_chain, task_to_column) -> Option<ColumnId>`. Not on `UIState` (which is Tier 0 / consumer-agnostic). Matches the precedent set by `swissarmyhammer_kanban::default_ui_state(app_subdir)`.

**Semantics.** Walks scope chain innermost-first; commits on first `column:*` or `task:*` moniker. `task:*` that misses the map returns `None` (does not silently fall through to an outer `column:*`) — caught by dedicated test.

**Dispatch wiring.** `AddEntityCmd::execute` calls `resolve_column_from_scope` when the caller didn't supply an explicit `column` arg. That helper gates the (async) task-map build on whether the chain actually contains a `task:*` moniker — `column:*`-only chains skip the `ectx.list("task")` entirely. Explicit `column` arg still wins (preserves the grid-view column-(+) button path).

**React delta.** Deleted `resolveFocusedColumnId`; `makeNewTaskCommand` now dispatches `entity.add:task` with no pre-computed `column`. `BoardActionDeps` loses its `taskMap` field (was only used by the deleted resolver).

**Files changed.**
- `swissarmyhammer-kanban/src/lib.rs` — register `pub mod focus;`
- `swissarmyhammer-kanban/src/focus.rs` — new, pure resolver + 4 unit tests
- `swissarmyhammer-kanban/tests/resolve_focused_column.rs` — new, 7 integration tests covering all branches
- `swissarmyhammer-kanban/src/commands/entity_commands.rs` — `AddEntityCmd` scope-chain resolution + 5 integration tests (column focused, task focused, nothing focused, explicit column override, unknown task-id in scope)
- `kanban-app/ui/src/components/board-view.tsx` — deleted `resolveFocusedColumnId`; simplified `makeNewTaskCommand` and `useBoardActionCommands` (drop `taskMap` threading)

**Test counts.** Rust: 7 integration + 4 unit (focus) + 5 integration (AddEntityCmd scope) = 16 new tests. UI: 1322 existing tests still pass (no regressions). Zero warnings in `cargo build --workspace`.

## Review Findings (2026-04-24 09:44)

### Nits

- [x] `swissarmyhammer-kanban/src/focus.rs:21-22` — Doc comment is internally contradictory: "the frontend populates index 0 with the focused entity and earlier indices with nested scopes". If index 0 is the focused entity, then *higher* (later) indices contain outer scopes, not "earlier" ones. Suggest: "the frontend populates index 0 with the focused entity and subsequent indices walk outward through nested scopes." The public-facing `# Parameters` doc on lines 38-40 already says "ordered innermost-first" cleanly — only the implementation aside on line 21-22 has the wording flipped.
  - **Resolution (2026-04-24)**: Flipped the wording in `focus.rs` as suggested — the implementation aside now reads "subsequent indices walk outward through nested scopes", matching the innermost-first semantics of the `# Parameters` doc and the actual resolver behavior.

- [x] `swissarmyhammer-kanban/src/commands/entity_commands.rs:86-88` — Minor style: the function signature uses fully-qualified `crate::types::ColumnId` while the body imports `use crate::types::{ColumnId, TaskId};` on line 88. Either drop the qualifier on line 86 (now that the import is right there) or move the import to the file's top. Both choices match the rest of the file better than the current mix.
  - **Resolution (2026-04-24)**: Moved both `use crate::focus::resolve_focused_column;` and `use crate::types::{ColumnId, TaskId};` to the file-top imports (where the other `crate::*` uses already live), and dropped the qualifier on `resolve_column_from_scope`'s return type — the file now has a single consistent import style.

- [x] `swissarmyhammer-kanban/src/commands/entity_commands.rs:32-67` — `AddEntityCmd::execute` synthesizes a `column` override for every entity type when the scope chain has a focused column/task — including types that do not declare `position_column` (actor, tag, project). `apply_position` silently drops it via the `has_position_column` check, so the behavior is correct, but the cost (resolving + an `ectx.list("task")` round-trip when `task:*` is in scope) is paid for nothing. Consider gating `resolve_column_from_scope` on whether the entity definition has a `position_column` field — would skip the lookup entirely for actors/tags/projects/columns. Not a correctness issue; pure perf/clarity nit.
  - **Resolution (2026-04-24)**: Added a `has_position_column` gate inside `resolve_column_from_scope`: after confirming the scope chain carries a column/task moniker (so the cheap entity-context read is actually needed), the helper now fetches the `EntityDef` via `ectx.fields().get_entity(entity_type)` and short-circuits with `Ok(None)` when the entity type does not declare `position_column`. The gate sits *before* the `ectx.list("task")` round-trip, so actors/tags/projects/columns/boards with a `task:*` in scope no longer pay that cost. The helper signature grew a `entity_type: &str` parameter threaded from `AddEntityCmd::execute`. Unknown entity types still fall through to `AddEntity`'s own "unknown entity type" error path rather than being masked here.

- [x] `swissarmyhammer-kanban/src/commands/entity_commands.rs` (test coverage) — No test asserts the "synthesized column is dropped for entities without position_column" path (e.g. `entity.add:actor` with `[column:doing]` in scope succeeds and the actor is created without a column). Currently this is implicit in the `apply_position` short-circuit. A small explicit test would document the contract and catch a regression if someone ever added `position_column` to actor.
  - **Resolution (2026-04-24)**: Added `add_entity_non_positional_drops_synthesized_column_silently` under `commands::entity_commands::tests`. It dispatches `entity.add:tag` (tags don't declare `position_column`) with scope chain `[column:doing, window:main]` and asserts (a) the command succeeds, (b) the resulting tag has no `position_column` field, and (c) the tag's schema-default `tag_name: "new-tag"` is present (sanity check that creation actually happened). The doc comment calls out that the test also guards the perf/clarity contract from nit #3 — if someone ever adds `position_column` to actor/tag/project, the assertion flips and surfaces the change.