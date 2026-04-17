---
assignees:
- claude-code
position_column: todo
position_ordinal: d980
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

- [ ] Review the full WIP diff — confirm every modified file's change is intentional and isolated to one of the two streams above.
- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands -p kanban-app` — all green (per 01KPCSY8R3413FB565CBA7PF9Z acceptance: 983/983 kanban, 71/71 kanban-app).
- [ ] `npx vitest run` in `kanban-app/ui/` — all green (per acceptance: 1183/1183).
- [ ] `cargo clippy -p swissarmyhammer-kanban -- -D warnings` — clean.
- [ ] Commit in logical chunks: (1) entity/add + position + mod.rs registry retirement, (2) SetActiveViewCmd fix + its tests, (3) scope_commands refactor + logging helpers, (4) any frontend tests / YAML micro-edits.
- [ ] `git status` clean before handoff.

## Acceptance Criteria

- [ ] `git status` shows no unstaged / untracked files touching commands, entities, scope, or ui_commands.
- [ ] All test suites listed above green.
- [ ] `register_commands_returns_expected_count` passes at 63.
- [ ] `task_add_not_registered_uses_entity_add_instead` and `project_add_not_registered_uses_entity_add_instead` pass.
- [ ] `set_active_view_rewrites_view_moniker_in_scope_chain` passes.
- [ ] Manual (from 01KPCSY8R3413FB565CBA7PF9Z): Tags grid + palette → "New Tag" appears; Projects grid + palette → "New Project" appears.

## Tests

- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands -p kanban-app` — all green.
- [ ] `npx vitest run` in `kanban-app/ui/` — all green.
- [ ] Manual: start `bun run tauri dev`, switch between Tasks / Tags / Projects grids, open palette, verify "New <Type>" item matches active grid.

## Workflow

- Use `/commit` or the `commit` skill — one focused commit per concern.
- No `/tdd` — code exists; this is review + commit.

#commands