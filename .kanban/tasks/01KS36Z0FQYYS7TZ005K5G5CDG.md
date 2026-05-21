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

Files to edit:
- Root `Cargo.toml` — remove `swissarmyhammer-commands` from workspace members
- Every other crate's `Cargo.toml` — remove the `swissarmyhammer-commands` dependency
- Every `use swissarmyhammer_commands::...` — delete or migrate to equivalent in `swissarmyhammer-command-service`

Pre-flight: before the deletion lands, every consumer of `Command` / `CommandRegistry` / `CommandContext` types from `swissarmyhammer-commands` must already be migrated. The forward-progress sequence:
1. New types in `swissarmyhammer-command-service` (already done in task
2. All 7 builtin command plugins live (already done — task-commands, kanban-misc-commands, file-commands, perspective-commands, entity-commands, ui-commands, app-shell-commands)
3. Frontend uses `useDispatchCommand` via Command service (already done)
4. THIS task: delete the old crate; fix every remaining compile error by either deleting dead code or porting to the new types

Memory `commands-in-rust`: kept — commands still live in Rust/plugin code. Memory `command-organization`: cross-cutting `entity.*` and `ui.*` lived once in entity.yaml/ui.yaml; those now live once in their respective builtin plugins.

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-commands/` does not exist
- [ ] No YAML files under `crates/swissarmyhammer-*/builtin/commands/` (all 12 deleted: 7 kanban-domain + 5 platform-shell)
- [ ] No `use swissarmyhammer_commands::` appears anywhere
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `apps/kanban-app` runs end-to-end: every command in the 62-command baseline works through the Command service
- [ ] No `swissarmyhammer-commands` in `Cargo.lock`

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/full_baseline_e2e.rs` — runs every YAML-defined command (from a checked-in baseline list of all 62 commands and their expected metadata, broken down by source YAML file) through the Command service; asserts every command is registered with the right metadata and that `execute` produces the same effect as the YAML-driven version did. This is the cut-over gate.
- [ ] `tests/no-stale-imports.rs` (or a CI grep step) — fail the build if `swissarmyhammer_commands::` appears in any non-deleted file
- [ ] `cargo build --workspace && cargo test --workspace` is the final acceptance

## Workflow
- Use `/tdd` — the `full_baseline_e2e.rs` test, written against the baseline command list, is the cut-over contract. Implement until it passes; then delete the old crate.

Depends on every builtin plugin port + the frontend dispatcher refactor + the Tauri replacement.