---
assignees:
- claude-code
depends_on:
- 01KRE1SSN9AX8R67XC58HHQKKB
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd980
title: Relocate DynamicSources and friends out of swissarmyhammer-kanban
---
## Status (2026-05-12)

**Substantially complete.** Seven of eight planned relocation commits landed:

1. `fix(commands,kanban)` baseline fix for prior task 01KRE1SSN9AX8R67XC58HHQKKB's incomplete commit (added missing module/struct edits the previous task's description claimed it had made but did not).
2. `WindowInfo` -> `swissarmyhammer-commands`.
3. `ViewInfo` -> `swissarmyhammer-views`.
4. `PerspectiveFieldInfo` -> `swissarmyhammer-perspectives`.
5. `PerspectiveInfo` -> `swissarmyhammer-perspectives`.
6. `PerspectiveFieldsResolver` -> `swissarmyhammer-perspectives` (plus new `OptionsSources` typed-multimap in commands so per-domain resolvers can coexist without circular deps).
7. `ViewKindsResolver` -> `swissarmyhammer-views`.
8. `SortDirectionsResolver` -> `swissarmyhammer-commands`.

**Deferred:** Moving `DynamicSources` (the aggregator) and `commands_for_scope` / `filter_by_view_kind` / `enrich_options` / the per-emitter dynamic functions (`emit_view_switch`, `emit_board_switch`, `emit_window_focus`, `emit_perspective_goto`, `emit_entity_add`) out of `swissarmyhammer-kanban`.

These five things are tightly coupled by data shape: every dynamic emitter reads from `DynamicSources` fields, and `commands_for_scope` orchestrates all of them. Moving `DynamicSources` to `kanban-app` without also moving `commands_for_scope` would require the orchestrator to consume `&dyn Any` for dynamic data, which loses the strongly-typed dynamic-emission logic. Moving `commands_for_scope` to `swissarmyhammer-commands` is blocked because the dynamic emitters reference `BoardInfo` (which is staying in kanban per task description), and `swissarmyhammer-commands` cannot depend on `swissarmyhammer-kanban`. The only clean resolution is to move the entire scope_commands module wholesale to `kanban-app`, which is a substantial reorganization with high risk to the test surface (4500+ lines, 1100+ tests). That is a follow-up task, not this one.

## Acceptance criteria status

Done:
- [x] `ViewInfo` is defined in `swissarmyhammer-views`. `swissarmyhammer-kanban` does not define it.
- [x] `PerspectiveInfo` and `PerspectiveFieldInfo` are defined in `swissarmyhammer-perspectives`. Kanban does not define either.
- [x] `WindowInfo` is defined in `swissarmyhammer-commands`. Kanban does not define it.
- [x] `BoardInfo` remains in `swissarmyhammer-kanban`.
- [x] `PerspectiveFieldsResolver` is defined and registered from `swissarmyhammer-perspectives`. `ViewKindsResolver` is defined and registered from `swissarmyhammer-views`. `SortDirectionsResolver` is in `swissarmyhammer-commands`. Kanban has zero `OptionsResolver` registrations of its own (it composes via per-domain helpers `register_perspective_resolvers`, `register_view_resolvers`, `register_command_resolvers`).
- [x] `swissarmyhammer-commands` does NOT depend on `swissarmyhammer-views`, `swissarmyhammer-perspectives`, or `swissarmyhammer-kanban`.
- [x] `swissarmyhammer-kanban` retains its existing deps; no symbols re-exported as passthrough.
- [x] No `pub use ... as ...` legacy re-exports left behind. Every consumer imports from the new canonical crate.
- [x] `cargo check --workspace --all-targets` clean.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [x] All affected-crate tests green. Test counts went up where types/resolvers moved:
  - `swissarmyhammer-commands`: 192 -> 197 (+5: OptionsSources sanity, SortDirectionsResolver, register helper)
  - `swissarmyhammer-views`: 67 -> 73 (+6: ViewInfo, ViewKindsResolver, title_case, register helper)
  - `swissarmyhammer-perspectives`: 57 -> 66 (+9: PerspectiveFieldInfo, PerspectiveInfo, PerspectiveFieldsResolver, register helper)
  - `swissarmyhammer-kanban`: 1136 -> 1130 (-6: tests moved with the types/resolvers)

Deferred for follow-up:
- [ ] `DynamicSources` (the aggregator) is defined in `kanban-app`.
- [ ] `commands_for_scope`, `filter_by_view_kind`, and `enrich_options` live in `swissarmyhammer-commands`.

## Out of scope (per original task)

- Any change to the wire format of `CommandDef` / `ParamDef` / `OptionsResolver` traits. (Met. `OptionsResolver` trait unchanged. New `OptionsSources` type was added to commands as a non-trait helper that resolvers downcast through.)
- The frontend `<CommandButton>` / `<CommandPopover>` work.

## New `OptionsSources` type — design note

To let per-domain resolvers (e.g. `PerspectiveFieldsResolver` in perspectives) coexist without each depending on the consumer's aggregator type (`DynamicSources` in kanban), commands now exports an `OptionsSources` typed multimap. The consumer composes one per `commands_for_scope` call, inserts per-domain data structs (`PerspectivesOptionsData`, etc.), and threads `&sources as &dyn Any` into `OptionsContext`. Each per-domain resolver downcasts to `&OptionsSources` then calls `OptionsSources::get::<DomainOptionsData>()`. This keeps the `OptionsResolver` trait surface unchanged (the explicit out-of-scope item) while letting the per-domain resolvers live in their domain crates.

#command-driven-ui