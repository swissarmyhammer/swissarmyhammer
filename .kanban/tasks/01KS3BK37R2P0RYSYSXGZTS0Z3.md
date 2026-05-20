---
assignees:
- claude-code
depends_on:
- 01KS36KNHH9BPC82MFMGTY3T5J
position_column: todo
position_ordinal: '9580'
project: command-service
title: 'Plugin catalog: enumerate every builtin plugin + lock cut-over baseline'
---
## What

Produce the locked-in catalog of every builtin command plugin we will create, with each plugin's directory, service dependencies, source YAML(s), and exact command roster. This is the contract: every plugin-port task implements against this catalog; the cut-over e2e test verifies against this catalog; any divergence is a bug.

Files to create:
- `crates/swissarmyhammer-command-service/tests/baseline/plugins.yaml` — checked-in YAML that mirrors the catalog table below in machine-readable form. Read by `full_baseline_e2e.rs` and metadata-fidelity tests.
- `crates/swissarmyhammer-command-service/tests/baseline/mod.rs` — Rust loader that parses `plugins.yaml` into typed `PluginSpec { dir, services, commands: Vec<CommandSpec> }`.

## The Catalog

**7 builtin plugins under `builtin/plugins/`. 62 commands total across 12 source YAML files.**

### 1. `task-commands` (3 commands)
- **dir**: `builtin/plugins/task-commands/`
- **ensureServices**: `["commands"]`
- **source YAML**: `crates/swissarmyhammer-kanban/builtin/commands/task.yaml`

| id | scope | undoable | context_menu | keys |
|----|-------|---------:|:------------:|------|
| `task.move` | `entity:task` | true | — | — |
| `task.untag` | `entity:tag,entity:task` | true | ✓ | vim:`x` cua:`Delete` |
| `task.doThisNext` | `entity:task` | true | ✓ | — |

### 2. `kanban-misc-commands` (5 commands — column + attachment + tag + view)
- **dir**: `builtin/plugins/kanban-misc-commands/`
- **ensureServices**: `["commands"]`
- **source YAMLs**: column.yaml, attachment.yaml, tag.yaml, view.yaml (kanban-domain)

| id | scope | undoable | context_menu | keys |
|----|-------|---------:|:------------:|------|
| `column.reorder` | — | true | — | — |
| `attachment.open` | `attachment` | — | ✓ | — |
| `attachment.reveal` | `attachment` | — | ✓ | — |
| `tag.update` | `entity:tag` | true | — | — |
| `view.set` | — | — | — | — |

### 3. `file-commands` (4 commands)
- **dir**: `builtin/plugins/file-commands/`
- **ensureServices**: `["commands", "window"]` (window for OS file dialog in `openBoard`)
- **source YAML**: file.yaml

| id | undoable | notes |
|----|---------:|-------|
| `file.switchBoard` | false | switches active board |
| `file.closeBoard` | false | closes active board |
| `file.newBoard` | false | new board file |
| `file.openBoard` | false | opens OS file picker |

### 4. `perspective-commands` (17 commands)
- **dir**: `builtin/plugins/perspective-commands/`
- **ensureServices**: `["commands"]`
- **source YAML**: perspective.yaml
- **subfile split** (keeps each helper <200 lines):
  - `commands/filter.ts` — `filter`, `filter.focus`, `clearFilter`
  - `commands/group.ts` — `group`, `clearGroup`
  - `commands/sort.ts` — `sort.set`, `sort.clear`, `sort.toggle`
  - `commands/nav.ts` — `next`, `prev`, `goto`, `switch`
  - `commands/lifecycle.ts` — `load`, `save`, `delete`, `rename`, `list`

| id | scope | undoable | context_menu |
|----|-------|---------:|:------------:|
| `perspective.load` | — | — | — |
| `perspective.save` | — | true | — |
| `perspective.delete` | `entity:perspective` | true | ✓ |
| `perspective.rename` | — | true | — |
| `perspective.filter.focus` | `entity:perspective` | — | — |
| `perspective.filter` | `entity:perspective` | true | — |
| `perspective.clearFilter` | `entity:perspective` | true | ✓ |
| `perspective.group` | `entity:perspective` | true | — |
| `perspective.clearGroup` | `entity:perspective` | true | ✓ |
| `perspective.sort.set` | `entity:perspective` | true | — |
| `perspective.sort.clear` | `entity:perspective` | true | ✓ |
| `perspective.sort.toggle` | `entity:perspective` | true | — |
| `perspective.next` | — | — | — |
| `perspective.prev` | — | — | — |
| `perspective.goto` | — | — | — |
| `perspective.list` | — | — | — |
| `perspective.switch` | — | — | — |

### 5. `entity-commands` (8 commands)
- **dir**: `builtin/plugins/entity-commands/`
- **ensureServices**: `["commands"]`
- **source YAML**: entity.yaml
- **note**: cross-cutting — uses `from: target` not `from: scope_chain`. Dynamic `entity.add:type` variants synthesized client-side by the palette from the entity-type registry; not registered as discrete commands.

| id | undoable | context_menu | notes |
|----|---------:|:------------:|-------|
| `entity.add` | true | — | dynamic per-type variants in palette |
| `entity.update_field` | true | — | — |
| `entity.delete` | true | ✓ | — |
| `entity.archive` | true | ✓ | — |
| `entity.unarchive` | true | ✓ | — |
| `entity.cut` | true | ✓ | clipboard |
| `entity.copy` | false | ✓ | clipboard |
| `entity.paste` | true | ✓ | clipboard, uses PasteMatrix |

### 6. `ui-commands` (10 commands incl. `window.new`)
- **dir**: `builtin/plugins/ui-commands/`
- **ensureServices**: `["commands", "window"]` (window for `window.new`)
- **source YAML**: ui.yaml

| id | scope | undoable | context_menu |
|----|-------|---------:|:------------:|
| `ui.inspect` | — | — | ✓ |
| `ui.inspector.close` | — | — | — |
| `ui.inspector.close_all` | — | — | — |
| `ui.inspector.set_width` | — | false | — |
| `ui.palette.open` | — | — | — |
| `ui.palette.close` | — | — | — |
| `ui.entity.startRename` | `entity:perspective` | — | — |
| `ui.mode.set` | — | false | — |
| `ui.setFocus` | — | false | — |
| `window.new` | — | — | — |

### 7. `app-shell-commands` (15 commands — app + settings + drag)
- **dir**: `builtin/plugins/app-shell-commands/`
- **ensureServices**: `["commands", "app"]` (app for quit/undo/redo)
- **source YAMLs**: app.yaml, settings.yaml, drag.yaml
- **subfile split**: `commands/app.ts`, `commands/settings.ts`, `commands/drag.ts`

| id | undoable | source |
|----|---------:|--------|
| `app.about` | — | app.yaml |
| `app.help` | — | app.yaml |
| `app.quit` | — | app.yaml |
| `app.command` | — | app.yaml |
| `app.palette` | — | app.yaml |
| `app.search` | — | app.yaml |
| `app.dismiss` | — | app.yaml |
| `app.undo` | false | app.yaml |
| `app.redo` | false | app.yaml |
| `settings.keymap.vim` | — | settings.yaml |
| `settings.keymap.cua` | — | settings.yaml |
| `settings.keymap.emacs` | — | settings.yaml |
| `drag.start` | false | drag.yaml |
| `drag.cancel` | false | drag.yaml |
| `drag.complete` | false | drag.yaml |

## Tallies (verification)

- Plugins: 7
- Source YAMLs: 12 (7 kanban-domain + 5 platform-shell)
- Commands: 3 + 5 + 4 + 17 + 8 + 10 + 15 = **62**

## Acceptance Criteria
- [ ] `crates/swissarmyhammer-command-service/tests/baseline/plugins.yaml` exists with all 7 plugins and 62 commands and their full metadata (id, name, scope, keys, undoable, context_menu, params, menu/tab_button if present)
- [ ] `baseline/mod.rs` loads the file into typed structs
- [ ] A baseline self-check test verifies tallies: 7 plugin specs; 12 source files referenced; 62 commands; every command id is unique across plugins; every plugin's command count matches its declared list
- [ ] A baseline-vs-YAML drift test reads every source YAML and asserts the catalog references every command in those YAMLs (and no extras) — fails CI if anyone adds a YAML command without updating the catalog

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/baseline/catalog_self_check.rs` — load `plugins.yaml`; assert tallies (7/12/62) and uniqueness
- [ ] `crates/swissarmyhammer-command-service/tests/baseline/yaml_vs_catalog.rs` — walk source YAML files, parse command ids, assert the set equals the catalog's command id set
- [ ] `cargo test -p swissarmyhammer-command-service --test baseline` passes

## Workflow
- Use `/tdd` — write the self-check + drift tests first; populate the catalog YAML until they pass. This task is the cut-over contract: once the catalog is checked in, every port task implements against it and the cut-over `full_baseline_e2e.rs` reads from it.

This task gates every plugin-port task — it must complete (catalog file checked in) before per-plugin work begins.