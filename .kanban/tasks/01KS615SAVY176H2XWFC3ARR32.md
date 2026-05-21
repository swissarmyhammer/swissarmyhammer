---
assignees:
- claude-code
depends_on:
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5E9M7ZNPNA0E7GR1C9N42R
- 01KS5EAD57PCBFJGMVB74FF4MK
position_column: todo
position_ordinal: a380
project: command-cutover
title: 'Pre-flight: migrate non-plugin Rust consumers off `swissarmyhammer-commands`'
---
## What

Before the crate-deletion task can compile, every NON-plugin Rust consumer of `swissarmyhammer-commands` must be migrated off it. The cut-over plan previously only named `UIState` + `window_info`; a workspace grep found four more crates depending on the crate's `Command`/`CommandContext`/`CommandError` traits and its `OptionsRegistry`/`OptionsResolver` machinery. Deleting the crate without handling these breaks the build. This task resolves each consumer; the deletion task depends on it.

Consumers found (verified by grepping `swissarmyhammer_commands` / `swissarmyhammer-commands` across Cargo.toml + `use` sites):
1. `crates/swissarmyhammer-entity/src/undo_commands.rs` — imports `Command, CommandContext, CommandError` (`:18`) and `ui_state::UIState` (`:124`). Cargo dep `crates/swissarmyhammer-entity/Cargo.toml:22`. These are the entity-layer `UndoCmd`/`RedoCmd`.
2. `crates/swissarmyhammer-kanban/src/commands/app_commands.rs` — the `Command` trait + `KanbanUndoCmd`/`KanbanRedoCmd`. **CRITICAL: `reconcile_post_undo_caches` (`:284`) MUST SURVIVE** — it is the undo→edit convergence point (see the command-events change-propagation task). Whatever happens to the trait-based command structs, the reconcile logic must be preserved/relocated, not deleted.
3. `crates/swissarmyhammer-views/src/options_resolvers.rs` — `OptionsContext`/`OptionsRegistry`/`OptionsResolver`/`ParamOption` (`:14`). Cargo dep `Cargo.toml:19`.
4. `crates/swissarmyhammer-perspectives/src/options_resolvers.rs` — same machinery (`:20`). Cargo dep `Cargo.toml:20`.
5. `crates/swissarmyhammer-focus/tests/nav_yaml.rs` — `CommandDef` (`:18`) + YAML loader. Cargo dep `Cargo.toml:22`.

### Decide per consumer (relocate vs migrate vs delete)

This is a design decision, not a mechanical move — make it explicitly for each, with the constraint that the workspace builds and `reconcile_post_undo_caches` survives:
- **Undo/redo command structs** (entity `UndoCmd`/`RedoCmd`, kanban `KanbanUndoCmd`/`KanbanRedoCmd`): in the new world, `app.undo`/`app.redo` are plugin commands routing to the `store` server. The Rust trait-based structs likely become dead and are DELETED — but only after confirming nothing else dispatches them, and after the reconcile is relocated.
- **`reconcile_post_undo_caches` + `reconcile_perspective_cache`/`reconcile_view_cache`**: relocate to a surviving home (the store-service/entity-service wiring, or kanban) so undo still reconciles caches. Do NOT lose it.
- **`OptionsRegistry`/`OptionsResolver`/`ParamOption`/`OptionsContext`** (dynamic param option sources used by views/perspectives): decide where dynamic-option resolution lives in the plugin world (plugin-declared params/option providers) and relocate or replace the machinery accordingly so views/perspectives/focus compile.
- **`CommandDef` + YAML loader in the focus test**: update or delete the test as the YAML command model goes away.

## Acceptance Criteria
- [ ] No crate other than `swissarmyhammer-commands` itself imports `swissarmyhammer_commands::{Command, CommandContext, CommandError, OptionsRegistry, OptionsResolver, OptionsContext, ParamOption, CommandDef}` (or any other item) after this task
- [ ] `reconcile_post_undo_caches` (and the perspective/view reconcile helpers) live in a surviving crate and still run after undo/redo
- [ ] `swissarmyhammer-entity`, `swissarmyhammer-views`, `swissarmyhammer-perspectives`, `swissarmyhammer-focus` all drop their `swissarmyhammer-commands` Cargo dependency
- [ ] `cargo build --workspace` succeeds with `swissarmyhammer-commands` still present but no longer referenced by these four crates (proves they're decoupled before deletion)

## Tests
- [ ] `tests/no-stale-imports.rs` (or CI grep) — fail if `swissarmyhammer_commands::` appears in `swissarmyhammer-entity`/`-views`/`-perspectives`/`-focus`
- [ ] An undo-reconcile regression test (reuse the command-events `undo_redo_notifies_dependents_e2e.rs`) still passes from the relocated reconcile home
- [ ] `cargo build --workspace && cargo test --workspace` passes

## Workflow
- Use `/tdd` — write the no-stale-imports grep test scoped to these four crates first; migrate until green.

Depends on the new homes existing: `store` server (undo/redo), `ui-state` server (UIState relocation), `entity` server (kernel reads). Blocks the crate-deletion task.