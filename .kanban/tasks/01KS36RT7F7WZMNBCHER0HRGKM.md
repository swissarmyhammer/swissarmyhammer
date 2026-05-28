---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS5EA17K4KDANFFRGW92QARF
position_column: review
position_ordinal: '80'
project: builtin-commands
title: 'Builtin plugin: column/attachment/tag/view commands (port 4 small YAMLs)'
---
## What

Port the four small kanban-domain YAML files to one builtin TypeScript plugin (5 commands).

Source YAMLs: `column.yaml` (`column.reorder`), `attachment.yaml` (`attachment.open`, `attachment.reveal`), `tag.yaml` (`tag.update`), `view.yaml` (`view.set`).

Files:
- `builtin/plugins/kanban-misc-commands/index.ts` — `load()` calls `ensureServices(this, ["commands", "kanban", "window", "views"])` then `registerCommands(this, [...])`.

Backend routing:
- `column.reorder` → **kanban** `update column` (positioning is a kanban domain op)
- `tag.update` → **kanban** `update tag` (kanban keeps its typed domain ops; it delegates generic CRUD to the entity kernel internally)
- `attachment.open` → **window** `OpenPath`; `attachment.reveal` → **window** `RevealPath`
- `view.set` → **views** `SetView`

Each registration preserves the YAML metadata 1:1 (keys, scope, params, undoable, context_menu, tab_button).

## Acceptance Criteria
- [ ] `builtin/plugins/kanban-misc-commands/` discoverable
- [ ] All 5 commands registered with metadata matching the YAML baselines
- [ ] Each routes to the backend above and produces the same observable effect as today
- [ ] `load()` calls `ensureServices` before `registerCommands`

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_kanban_misc_e2e.rs` — load; assert 5 registered with metadata; execute each and observe effect (column reorder + tag.update via kanban, attachment via window, view.set via views)
- [ ] Metadata-fidelity tests per command
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_kanban_misc_e2e` passes

## Workflow
- Use `/tdd` — metadata fidelity first.