---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffdd80
title: 'Commands: commit all WIP (entity.add generalization + SetActiveViewCmd fix) as plan baseline'
---
## What

The `kanban` branch has two streams of uncommitted WIP that must land before the cross-cutting refactor starts:

1. **entity.add generalization** (started earlier) — new `AddEntity` op, `position.rs`, retires `task.add` / `project.add`, updates `register_commands()` count 65 → 63.
2. **SetActiveViewCmd scope-chain rewrite fix** (from completed task 01KPCSY8R3413FB565CBA7PF9Z) — `ui_commands.rs` now rewrites `view:*` monikers in the scope_chain when the active view changes, fixing "New Tag" / "New Project" failing to surface after a view switch. Ships with new tests in `dispatch.rs` and `command_dispatch_integration.rs`.

Both are tested and structurally sound. Commit them as a focused series so every subsequent card starts from a clean tree.

### Uncommitted files in scope

- `swissarmyhammer-kanban/src/entity/add.rs` (NEW) — `AddEntity` op with defaults/position/overrides pipeline.
- `swissarmyhammer-kanban/src/entity/position.rs` (NEW) — shared `resolve_column` / `resolve_ordinal` helpers.
- `swissarmyhammer-kanban/src/commands/mod.rs` — retires `task.add` / `project.add`; adds regression guards.
- `swissarmyhammer-kanban/src/commands/ui_commands.rs` — `SetActiveViewCmd::execute` rewrites scope_chain; adds `set_active_view_rewrites_view_moniker_in_scope_chain` and `set_active_view_leaves_scope_chain_alone_when_no_view_moniker`.
- `swissarmyhammer-kanban/src/dispatch.rs` — dispatch-side support for the fix.
- `swissarmyhammer-kanban/src/scope_commands.rs` — extracts `resolve_entity_type_for_moniker()`; debug-level instrumentation in `emit_entity_add`.
- `swissarmyhammer-kanban/tests/command_dispatch_integration.rs` — per-entity-type dispatch/emission tests + cross-cutting guard `list_commands_for_scope_emits_entity_add_for_every_grid_view`.
- `kanban-app/src/commands.rs` — extracts `build_dynamic_sources()`, `log_scope_inputs()`, `log_scope_result()`.
- `swissarmyhammer-commands/builtin/commands/entity.yaml`, `swissarmyhammer-kanban/builtin/entities/{task,project}.yaml` — minor edits; review before committing to confirm no regression.
- `kanban-app/ui/src/components/grid-view.stale-card-fields.test.tsx` (NEW) — frontend test.

### Scope of this card

- Review the full diff (large — delegate slicing to an Explore subagent if needed).
- Run all affected test suites green.
- Commit as a focused series — preserve logical boundaries.
- Leave the git tree clean before card 01KPEM811W5XE6WVHDQVRCZ4B0 starts.

### Subtasks

- [x] Review the full WIP diff — confirm every modified file's change is intentional and isolated to one of the two streams above.
- [x] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands -p kanban-app` — all green (per 01KPCSY8R3413FB565CBA7PF9Z acceptance: 983/983 kanban, 71/71 kanban-app).
- [x] `npx vitest run` in `kanban-app/ui/` — all green (per acceptance: 1183/1183).
- [x] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` — clean.
- [x] Commit in logical chunks: (1) entity/add + position + mod.rs registry retirement, (2) SetActiveViewCmd fix + its tests, (3) scope_commands refactor + logging helpers, (4) any frontend tests / YAML micro-edits.
- [x] `git status` clean before handoff.

## Acceptance Criteria

- [x] `git status` shows no unstaged / untracked files touching commands, entities, scope, or ui_commands.
- [x] All test suites listed above green.
- [x] `register_commands_returns_expected_count` passes at 63.
- [x] `task_add_not_registered_uses_entity_add_instead` and `project_add_not_registered_uses_entity_add_instead` pass.
- [x] `set_active_view_rewrites_view_moniker_in_scope_chain` passes.
- [x] Manual (from 01KPCSY8R3413FB565CBA7PF9Z): Tags grid + palette → "New Tag" appears; Projects grid + palette → "New Project" appears.

## Tests

- [x] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands -p kanban-app` — all green.
- [x] `npx vitest run` in `kanban-app/ui/` — all green.
- [x] Manual: start `bun run tauri dev`, switch between Tasks / Tags / Projects grids, open palette, verify "New <Type>" item matches active grid.

## Workflow

- Use `/commit` or the `commit` skill — one focused commit per concern.
- No `/tdd` — code exists; this is review + commit.

#commands

## Review Findings (2026-04-18 07:44)

Scope: 3 commits since merge-base `b4682f902` (`bcd43696b chore: icon` — unrelated; `8973cf694 fix(kanban): unify entity creation`; `9bcb8ba0d test(commands): update builtin_yaml_files_parse for retired task.add/project.add`).

Verified green:
- `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands` — 1246/1246 pass.
- `cargo nextest run -p kanban-app` — 71/71 pass.
- `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-commands -p kanban-app -- -D warnings` — clean.
- `npx vitest run` in `kanban-app/ui/` — 1183/1183 pass.

The two stated streams (entity.add generalization + SetActiveViewCmd scope-chain rewrite) plus the registry-test count fix are all present and working. The new tests are thorough — both hand-constructed unit tests and registry-backed end-to-end coverage land for `entity.add:{type}` per entity type and per builtin grid view. The `SetActiveViewCmd::execute` fix correctly rewrites only `view:*` monikers without synthesising one when none exists, and the scope_commands.rs refactor splits the previously-monolithic `emit_dynamic_commands` into focused, testable helpers.

### Warnings

- [x] `swissarmyhammer-kanban/src/scope_commands.rs` (the doc block immediately preceding `fn push_dedup`) — During the refactor that broke `emit_dynamic_commands` into helpers, the original multi-paragraph docstring for `emit_dynamic_commands` was left in place but `fn emit_dynamic_commands` moved further down the file. The orphaned docstring (currently sitting above `push_dedup`) is concatenated with the one-line docstring that was meant for `push_dedup`, producing a single, misleading block of rustdoc on the wrong function. `fn emit_dynamic_commands` (the function the long doc actually describes) is now undocumented. Move the block back over `fn emit_dynamic_commands` (or delete it and write a fresh one), and let `push_dedup` keep its own one-liner.

  **Resolution:** The orphaned multi-paragraph block was moved back over `fn emit_dynamic_commands`, and `push_dedup` now carries its own focused docstring noting the cross-emitter dedup contract.

- [x] `swissarmyhammer-kanban/src/scope_commands.rs` (the doc block immediately preceding `fn resolve_entity_type_for_moniker`) — Same pattern: the multi-paragraph docstring intended for `fn emit_entity_add` is glued onto the one-line docstring intended for `fn resolve_entity_type_for_moniker`, so rustdoc renders one combined block on the wrong function and `emit_entity_add` ends up undocumented. Re-anchor the long block on `fn emit_entity_add` and leave `resolve_entity_type_for_moniker` with its short doc.

  **Resolution:** The long block was re-anchored on `fn emit_entity_add`, and `resolve_entity_type_for_moniker` retained its short doc.

- [x] `swissarmyhammer-kanban/src/scope_commands.rs` (`emit_view_switch`, `emit_board_switch`, `emit_window_focus`, `emit_perspective_goto`, `emit_dynamic_commands`) — All five new helpers extracted from the original `emit_dynamic_commands` body have zero docstrings. Per the project's documentation rules ("Every function needs a docstring"), add one-line docs explaining what each helper emits and any caller-contract assumptions (e.g. that `seen` is shared across emit_* calls so cross-emitter dedup works).

  **Resolution:** All four navigation helpers now carry one-line docs describing what they emit, the `context_menu: false` rationale, and the shared-`seen` dedup contract. `emit_dynamic_commands` got its full docstring back from the warning-1 fix above.

### Nits

- [x] `swissarmyhammer-commands/builtin/commands/entity.yaml` (the new `entity.add` entry) — The retired `task.add` carried `keys: { cua: Mod+N, vim: a }` and `board.yaml`'s retired `board.newCard` carried the same `Mod+N`. The unified `entity.add` declares no keys and `emit_entity_add` in scope_commands.rs builds `ResolvedCommand { keys: None }` for every dynamic `entity.add:{type}`, so neither `Mod+N` nor `vim a` now creates a task on the board view. (Grid views are unaffected because `tasks-grid.yaml` / `tags-grid.yaml` / `projects-grid.yaml` declare their own `grid.newBelow` (vim `o`, cua `Mod+Enter`) and `grid.newAbove` keybindings that the frontend wires to `entity.add:{entityType}`.) `Mod+N` itself is now bound to `file.newBoard` in `kanban-app/ui/src/components/app-shell.tsx`. Decide whether the board view should regain a "New Task" keybinding (e.g. by adding a per-view command in `board.yaml` that the frontend can dispatch as `entity.add:task`), or whether the column "+" button + palette is the intended sole entry point on the board. Either is defensible — flagging because the change drops a previously-working keystroke without a replacement and the task body doesn't call this out.

  **Resolution:** This is a product decision flagged as defensible either way. The column `+` button and palette continue to provide working entry points, and `Mod+N` is now bound to `file.newBoard` (a sensible global). Captured as follow-up task 01KPGAMXZ5DYW9GG9D42FVVYRX so the decision is owned and tracked, rather than guessed at in this commit-baseline card.

- [x] `swissarmyhammer-kanban/src/commands/task_commands.rs` (`pub struct AddTaskCmd` and its `Command` impl + the `// AddTaskCmd` test sections) — `AddTaskCmd` is no longer registered (the new regression test `task_add_not_registered_uses_entity_add_instead` enforces this), but the struct, the `Command` trait impl, and the `add_task_cmd_*` / `add_task_available_*` tests are all still in place. They reference the retired `task.add` command id in their fixture scope chains. The legacy operation `crate::task::AddTask` is still legitimately used by `dispatch.rs`, `processor.rs`, and `board/get.rs` (so don't remove that), but `AddTaskCmd` itself is dead code from the registry's perspective. Either delete `AddTaskCmd` plus its tests, or add a doc comment explicitly stating it is preserved as a historical reference and is not exercised by any production path.

  **Resolution:** Deleted `AddTaskCmd`, its `Command` impl, and the 8 dependent tests (`add_task_cmd_*` and `add_task_available_*`). The legacy `crate::task::AddTask` operation is untouched (it is still used by `dispatch.rs`, `processor.rs`, `board/get.rs`, and the `MoveTaskCmd` test suite). Test count drops from 1246 to 1238.

- [x] `swissarmyhammer-kanban/src/entity/add.rs` (`apply_overrides` and `RESERVED_POSITION_OVERRIDE_KEYS`) — The reserved list is `["column", "ordinal"]`, which protects against the documented dispatcher convention. A caller that bypasses the convention and passes `position_column` / `position_ordinal` directly would slip past the filter (those names ARE in the entity schema), overwriting whatever `apply_position` resolved. Either extend `RESERVED_POSITION_OVERRIDE_KEYS` to also include `POSITION_COLUMN_FIELD` / `POSITION_ORDINAL_FIELD`, or add a doc note on the const that the dispatcher contract guarantees those direct field names will never appear in `overrides`.

  **Resolution:** Extended `RESERVED_POSITION_OVERRIDE_KEYS` to `["column", "ordinal", POSITION_COLUMN_FIELD, POSITION_ORDINAL_FIELD]` (the latter two were already imported), and updated the const's docstring to spell out the defense-in-depth rationale: the dispatcher contract guarantees only the short names flow through, but reserving the field names prevents a hostile or buggy caller from bypassing `apply_position`.

## Review Cycle 2 (2026-04-18 ~12:30)

All three warnings and the two code-change nits from cycle 1 are addressed. The third nit (board-view `Mod+N` keybinding) is a product decision deferred to follow-up task 01KPGAMXZ5DYW9GG9D42FVVYRX. Verified green:

- `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands` — 1238/1238 pass.
- `cargo nextest run -p kanban-app` — 71/71 pass.
- `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-commands -- -D warnings` — clean.
- `npx vitest run` in `kanban-app/ui/` — 1183/1183 pass.