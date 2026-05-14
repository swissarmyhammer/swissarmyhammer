---
assignees:
- claude-code
depends_on:
- 01KRE7VDF7RXHV39VPEVH23NN4
position_column: todo
position_ordinal: a980
title: Move DynamicSources and scope_commands to kanban-app
---
## What

Move the scope_commands module (`commands_for_scope`, `filter_by_view_kind`, `enrich_options`, the per-emitter helpers `emit_view_switch`/`emit_board_switch`/`emit_window_focus`/`emit_perspective_goto`/`emit_entity_add`/`emit_scoped_commands`/`emit_cross_cutting_commands`/`emit_scoped_registry_commands`/`emit_global_registry_commands`, plus `DynamicSources` and `ResolvedCommand`) out of `swissarmyhammer-kanban` and into `kanban-app`.

This is the deferred portion of 01KRE7VDF7RXHV39VPEVH23NN4. The Info types (`WindowInfo`, `ViewInfo`, `PerspectiveInfo`, `PerspectiveFieldInfo`) and the three built-in resolvers (`PerspectiveFieldsResolver`, `ViewKindsResolver`, `SortDirectionsResolver`) already migrated cleanly. The aggregator + orchestrator + dynamic emissions form one tightly-coupled unit that needs to move together.

## Why this can't go to `swissarmyhammer-commands`

The dynamic emitters call `dyn_src.boards` and construct `board.switch:{path}` rows using `BoardInfo`. `BoardInfo` stays in `swissarmyhammer-kanban` (it is genuinely kanban-specific — boards are the kanban-board concept). `swissarmyhammer-commands` cannot depend on `swissarmyhammer-kanban` (the small-stable-base property the architecture relies on). So the orchestrator's home has to be a crate that depends on kanban — `kanban-app` is that place.

## Why this can't stay in `swissarmyhammer-kanban`

`DynamicSources` references types from every domain crate (perspectives, views, kanban). The original task description called this the "kanban knows everything" anti-pattern and resolved to move the aggregator above the domain crates in the dep graph. `kanban-app` already depends on every domain crate.

## Approach

Wholesale module move:

- Move `swissarmyhammer-kanban/src/scope_commands.rs` to `kanban-app/src/scope_commands.rs` (or a submodule).
- Move `swissarmyhammer-kanban/src/dynamic_sources.rs` to `kanban-app/src/dynamic_sources.rs`.
- `BoardInfo` stays in `swissarmyhammer-kanban` (re-exported by kanban-app's scope_commands).
- Test files that exercise `commands_for_scope` end-to-end move with the orchestrator:
  - `swissarmyhammer-kanban/tests/dynamic_sources_headless.rs`
  - `swissarmyhammer-kanban/tests/command_dispatch_integration.rs`
  - `swissarmyhammer-kanban/tests/command_snapshots.rs`
  - `swissarmyhammer-kanban/tests/command_surface_matrix.rs`
  - `swissarmyhammer-kanban/tests/options_enrichment.rs`
  - `swissarmyhammer-kanban/tests/tab_button_forwarding.rs`
  - `swissarmyhammer-kanban/tests/perspective_migration.rs` (if it uses DynamicSources)

Any kanban-internal test that uses `commands_for_scope` ends up either:
- migrating to kanban-app's test surface, or
- staying in kanban as a smaller unit test that doesn't need the orchestrator.

## Acceptance Criteria

- [ ] `swissarmyhammer-kanban` no longer defines `DynamicSources`, `commands_for_scope`, `filter_by_view_kind`, `enrich_options`, `ResolvedCommand`, or any of the dynamic-emit helpers.
- [ ] `kanban-app` defines all of them. Consumers (the kanban-cli, anyone else who wants to dispatch a scoped command) call `kanban_app::scope_commands::commands_for_scope`.
- [ ] `BoardInfo` stays in `swissarmyhammer-kanban` and is re-imported by kanban-app's scope_commands.
- [ ] No `pub use ... as ...` legacy re-exports in `swissarmyhammer-kanban` for any of the relocated symbols. Every caller imports from kanban-app.
- [ ] `cargo check --workspace --all-targets` clean.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo test --workspace --all-targets` green.
- [ ] The relocated tests retain their coverage at the new location.

## Tests

- [ ] Test counts in kanban-app go up by roughly the number of tests in the moved test files (~50-100 tests).
- [ ] Test counts in swissarmyhammer-kanban go down by the same amount.
- [ ] No test loses coverage. If a test exercised a now-deleted public surface, document why and update it.

## Workflow

- Do the move in two passes:
  1. Wholesale relocate scope_commands.rs + dynamic_sources.rs to kanban-app, fix imports, get cargo check green.
  2. Move every dependent test to kanban-app/tests/.
- Run `cargo check --workspace --all-targets` between every commit.
- This is a large mechanical refactor (4500+ lines moving, 1100+ tests potentially affected). Multiple commits is fine; one wholesale commit is also fine if cargo check stays green throughout.

## Sequencing

This task is BLOCKED by 01KRE7VDF7RXHV39VPEVH23NN4 (the Info-types and resolvers migration), which lands the OptionsSources plumbing this task depends on. Do not start until that one is in review/done.

#command-driven-ui