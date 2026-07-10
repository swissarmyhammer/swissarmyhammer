---
assignees:
- claude-code
depends_on:
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5E9M7ZNPNA0E7GR1C9N42R
- 01KS5EAD57PCBFJGMVB74FF4MK
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdd80
project: command-cutover
title: 'Pre-flight: migrate non-plugin Rust consumers off `swissarmyhammer-commands`'
---
## What

Before the crate-deletion task can compile, every NON-plugin Rust consumer of `swissarmyhammer-commands` must be migrated off it. (Original inventory: entity `undo_commands.rs`, kanban `app_commands.rs`, views/perspectives `options_resolvers.rs`, focus `nav_yaml.rs` test.)

## Acceptance Criteria
- [x] No crate other than `swissarmyhammer-commands` itself imports `swissarmyhammer_commands::{Command, CommandContext, CommandError, OptionsRegistry, OptionsResolver, OptionsContext, ParamOption, CommandDef}` — verified by the workspace-wide `no_stale_imports` gate + scoped grep (clean across entity/views/perspectives/focus).
- [x] `reconcile_post_undo_caches` (+ `reconcile_caches`) live in a surviving crate (`swissarmyhammer-kanban/src/commands/app_commands.rs`) and still run after undo/redo — covered by `undo_redo_notifies_dependents_e2e` (2/2 pass).
- [x] entity/views/perspectives/focus all dropped their `swissarmyhammer-commands` Cargo dependency.
- [x] `cargo build --workspace` succeeds. (AC premise exceeded: Stage 4 DELETED the `swissarmyhammer-commands` crate outright rather than leaving it present-but-unreferenced — `crates/swissarmyhammer-commands` no longer exists, zero Cargo refs.)

## Completion note (2026-06-03)
VERIFIED ALREADY DONE by the Stage 4 cut-over (commit `1377ee14f` + siblings) — this card's inventory was stale. Outcomes:
- entity `UndoCmd`/`RedoCmd` → DELETED (undo/redo route to the `store` server, which exposes `undo`/`redo` in `handle.rs`).
- kanban `KanbanUndoCmd`/`KanbanRedoCmd` → retained on the inlined `crate::commands_core`; `reconcile_post_undo_caches` SURVIVES there.
- views + perspectives `OptionsRegistry`/`OptionsResolver`/`ParamOption`/`OptionsContext` → relocated to the new dedicated `swissarmyhammer-command-options` crate.
- focus `nav_yaml.rs` test → updated to a local `NavCommandDef` + `builtin_yaml_sources()`; no dead-crate dep.

Zero code changes this session; verification only. Tests: `cargo build --workspace` pass; `no_stale_imports` 1/1; undo-reconcile e2e 2/2; in-scope crates green. Unblocks the crate-deletion task `01KS36Z0FQYYS7TZ005K5G5CDG`.

Discovered pre-existing, UNRELATED failures (not caused by this card, not in scope): `claude-agent` `session::tests` (~23, CWD/test-isolation `NotFound` panics) and `swissarmyhammer-focus` `meta_snapshot::focus_tool_meta_operations_tree_is_complete` (`generate sneak_codes` in inputSchema enum but absent from the `_meta` tree). Plus the two long-known kanban failures (`test_no_orphan_rust_commands_without_yaml`, `derive_handlers::apply_normalizes_slugs`).