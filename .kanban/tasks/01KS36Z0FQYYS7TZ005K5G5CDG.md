---
assignees:
- claude-code
depends_on:
- 01KS36RBS1KB6T21ENB9X7H14M
- 01KS36RT7F7WZMNBCHER0HRGKM
- 01KS36SEXMBGZJTWJX0ZQQKP8V
- 01KS36SWFYJRPQHD073FTRZYAE
- 01KS36TCNMSDGSQBZP3NKY6YK7
- 01KS36TSWE3NR5MFQTY99JX5TB
- 01KS36V80DXK2BFDDSHSWP131W
- 01KS36XGKCQ36QM7P6MH3FHMBJ
- 01KS36Y4NBDZMGH6QF963MD6FE
- 01KS5E9M7ZNPNA0E7GR1C9N42R
- 01KS5EA17K4KDANFFRGW92QARF
- 01KS5EAD57PCBFJGMVB74FF4MK
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS36W7VTKXXS4Z1C0P4SHZDT
- 01KS5F5ZNA0621X8KM2NPERXNV
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5F8THM5EQMKFSF6GFAE55C
- 01KS5G3AKZXDN7K6YR415E0V4K
- 01KS5G3S1MR6Y77RXPHZP4SZB1
- 01KS615SAVY176H2XWFC3ARR32
- 01KS614S1YAVEWVR1RHP62SQF0
- 01KS61511W6EGZ88043S261RSH
- 01KS612DV4W0N1X1RPXWAKMT4B
- 01KS613VPH2G4ZWKZPGW9ZCJAA
position_column: todo
position_ordinal: '9380'
project: command-cutover
title: 'Cut-over: delete `swissarmyhammer-commands` crate + YAML files + loader'
---
## What

The big-bang final step: delete the entire `swissarmyhammer-commands` Rust crate (Command trait, registry, YAML loader, context, options_resolver, ui_state, window_info, all 5 cross-cutting YAML files) and the 7 kanban-domain YAML files. Update the workspace `Cargo.toml`, every consumer's `Cargo.toml`, and every `use swissarmyhammer_commands::*` in the codebase.

Files to delete (total: 12 YAML files, 62 commands):
- `crates/swissarmyhammer-commands/` (entire crate, includes 5 YAML files: app.yaml, drag.yaml, entity.yaml, settings.yaml, ui.yaml)
- `crates/swissarmyhammer-kanban/builtin/commands/*.yaml` (7 files: attachment.yaml, column.yaml, file.yaml, perspective.yaml, tag.yaml, task.yaml, view.yaml)
- Any code in `swissarmyhammer-kanban` that loaded those YAMLs

NOT deleted here: `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` (the 9 `nav.*` commands) — owned by the spatial-nav project, handled separately.

Files to edit:
- Root `Cargo.toml` — remove `swissarmyhammer-commands` from workspace members
- Every other crate's `Cargo.toml` — remove the `swissarmyhammer-commands` dependency
- Every `use swissarmyhammer_commands::...` — delete or migrate to equivalent in `swissarmyhammer-command-service`

Pre-flight (forward-progress sequence) — ALL must already be done before the deletion lands:
1. New types in `swissarmyhammer-command-service` (the engine tasks).
2. All 7 builtin command plugins live (task-commands, kanban-misc-commands, file-commands, perspective-commands, entity-commands, ui-commands, app-shell-commands).
3. Frontend uses `useDispatchCommand` via the Command service + the non-transport `invoke()` migration.
4. **Non-plugin Rust consumers migrated off the crate** — the dedicated pre-flight task (`01KS615SAVY176H2XWFC3ARR32`) handles `swissarmyhammer-entity` (undo_commands), `swissarmyhammer-views`/`swissarmyhammer-perspectives` (`OptionsRegistry`/`OptionsResolver`), and `swissarmyhammer-focus` (test), and relocates `UIState` (ui-state server task), `window_info`/`WindowInfo` (window server task), and the `reconcile_post_undo_caches` convergence logic. This task depends on it; do not delete the crate until that task is green.
5. THIS task: delete the old crate; fix any remaining compile error by deleting dead code.

Memory `commands-in-rust`: kept — commands still live in Rust/plugin code. Memory `command-organization`: cross-cutting `entity.*` and `ui.*` lived once in entity.yaml/ui.yaml; those now live once in their respective builtin plugins.

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-commands/` does not exist
- [ ] No YAML files under `crates/swissarmyhammer-*/builtin/commands/` EXCEPT `swissarmyhammer-focus/.../nav.yaml` (the 12 command YAMLs deleted: 7 kanban-domain + 5 platform-shell; nav.yaml stays)
- [ ] No `use swissarmyhammer_commands::` appears anywhere
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `apps/kanban-app` runs end-to-end: every command in the 62-command baseline works through the Command service
- [ ] No `swissarmyhammer-commands` in `Cargo.lock`

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/full_baseline_e2e.rs` — runs every YAML-defined command (from the checked-in baseline catalog of all 62 commands + expected metadata, by source YAML) through the Command service; asserts every command is registered with the right metadata and that `execute` produces the same effect as the YAML-driven version. The cut-over gate.
- [ ] `tests/no-stale-imports.rs` (or a CI grep step) — fail the build if `swissarmyhammer_commands::` appears in any non-deleted file
- [ ] `cargo build --workspace && cargo test --workspace` is the final acceptance

## Workflow
- Use `/tdd` — the `full_baseline_e2e.rs` test, written against the baseline catalog, is the cut-over contract. Implement until it passes; then delete the old crate.

Depends on every builtin plugin port + the frontend dispatcher refactor + the non-transport invoke migration + the pre-flight consumer-migration task (`01KS615SAVY176H2XWFC3ARR32`).