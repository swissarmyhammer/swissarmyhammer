---
assignees:
- claude-code
depends_on:
- 01KS36KNHH9BPC82MFMGTY3T5J
position_column: todo
position_ordinal: '9580'
project: builtin-commands
title: 'Plugin catalog: enumerate every builtin plugin + lock cut-over baseline'
---
## What

Locked-in catalog of every builtin command plugin: directory, `ensureServices`, the backend MCP server each command calls, source YAML(s), and exact command roster. Contract for every plugin-port task and the cut-over e2e test.

Files to create:
- `crates/swissarmyhammer-command-service/tests/baseline/plugins.yaml` — machine-readable mirror of this catalog.
- `crates/swissarmyhammer-command-service/tests/baseline/mod.rs` — loader → typed `PluginSpec { dir, ensure_services, backends, commands }`.

## Backend services (where the work happens)

Only `kanban` exists today; the rest are net-new server tasks. All store-backed servers share ONE `Arc<StoreContext>` (the single undo substrate).

| Server | Status | Backs |
| ------ | ------ | ----- |
| `commands` | built here | the registry every plugin registers into |
| `kanban` | EXISTS (+extended) | task/column/tag/project/attachment CRUD, move/complete/tag/untag; **+ archive/unarchive + cut/copy/paste** |
| `views` | NEW | perspective.* + view.set |
| `ui_state` | NEW | ui.* + settings.keymap.* + drag.* + app.command/palette/search/dismiss |
| `window` | NEW | window.new + file.* board lifecycle + attachment.open/reveal |
| `app` | NEW | app.quit/about/help only (app-shell) |
| `store` | NEW | **undo/redo** (unified stack), transaction grouping, per-store history; store-scoped ops take a `store` param |

`undoable` flag note: it is **declarative metadata**, not the undo gate. Undo is recorded at the store layer on every tracked write; `store.undo` reverts the last entry/group across whatever stores it touched. Change propagation: undo emits the same thin events as edits, so caches + UI react through their normal paths (see the change-propagation task).

## The 7 plugins

### 1. `task-commands` — 3 — ensureServices `[commands, kanban]` — task.yaml
| id | scope | undoable | ctx_menu | keys | backend |
|----|-------|---------:|:--------:|------|---------|
| `task.move` | `entity:task` | true | — | — | kanban `move task` |
| `task.untag` | `entity:tag,entity:task` | true | ✓ | vim:`x` cua:`Delete` | kanban `untag task` |
| `task.doThisNext` | `entity:task` | true | ✓ | — | kanban `next task` |

### 2. `kanban-misc-commands` — 5 — ensureServices `[commands, kanban, window, views]` — column/attachment/tag/view.yaml
| id | scope | undoable | ctx_menu | backend |
|----|-------|---------:|:--------:|---------|
| `column.reorder` | — | true | — | kanban `update column` |
| `attachment.open` | `attachment` | — | ✓ | window `OpenPath` |
| `attachment.reveal` | `attachment` | — | ✓ | window `RevealPath` |
| `tag.update` | `entity:tag` | true | — | kanban `update tag` |
| `view.set` | — | — | — | views `SetView` |

### 3. `file-commands` — 4 — ensureServices `[commands, window]` — file.yaml
`file.switchBoard/closeBoard/newBoard/openBoard` (all undoable:false) → window board lifecycle.

### 4. `perspective-commands` — 17 — ensureServices `[commands, views]` — perspective.yaml
All → `views`. subfiles: `commands/{filter,group,sort,nav,lifecycle}.ts`.
load, save(undoable), delete(scope perspective,undoable,ctx_menu), rename(undoable), filter.focus(scope), filter(scope,undoable), clearFilter(scope,undoable,ctx_menu), group(scope,undoable), clearGroup(scope,undoable,ctx_menu), sort.set(scope,undoable), sort.clear(scope,undoable,ctx_menu), sort.toggle(scope,undoable), next, prev, goto, list, switch.

### 5. `entity-commands` — 8 — ensureServices `[commands, kanban]` — entity.yaml
Cross-cutting, `from: target`; dynamic `entity.add:type` synthesized client-side.
| id | undoable | ctx_menu | backend |
|----|---------:|:--------:|---------|
| `entity.add` | true | — | kanban `add <type>` |
| `entity.update_field` | true | — | kanban `update <type>` |
| `entity.delete` | true | ✓ | kanban `delete <type>` |
| `entity.archive` | true | ✓ | kanban `archive task` (ext) |
| `entity.unarchive` | true | ✓ | kanban `unarchive task` (ext) |
| `entity.cut` | true | ✓ | kanban `cut` (ext) |
| `entity.copy` | false | ✓ | kanban `copy` (ext) |
| `entity.paste` | true | ✓ | kanban `paste` (ext) |

### 6. `ui-commands` — 10 — ensureServices `[commands, ui_state, window]` — ui.yaml
| id | scope | undoable | ctx_menu | backend |
|----|-------|---------:|:--------:|---------|
| `ui.inspect` | — | — | ✓ | ui_state `Inspect` |
| `ui.inspector.close` | — | — | — | ui_state `InspectorClose` |
| `ui.inspector.close_all` | — | — | — | ui_state `InspectorCloseAll` |
| `ui.inspector.set_width` | — | false | — | ui_state `InspectorSetWidth` |
| `ui.palette.open` | — | — | — | ui_state `PaletteOpen` |
| `ui.palette.close` | — | — | — | ui_state `PaletteClose` |
| `ui.entity.startRename` | `entity:perspective` | — | — | ui_state `StartRename` |
| `ui.mode.set` | — | false | — | ui_state `SetKeymapMode` |
| `ui.setFocus` | — | false | — | ui_state `SetFocus` |
| `window.new` | — | — | — | window `OpenNewWindow` |

### 7. `app-shell-commands` — 15 — ensureServices `[commands, app, ui_state, store]` — app/settings/drag.yaml
| id | undoable | backend |
|----|---------:|---------|
| `app.about` | — | app `ShowAbout` |
| `app.help` | — | app `ShowHelp` |
| `app.quit` | — | app `QuitApp` |
| `app.undo` | false | **store `Undo`** (unified stack) |
| `app.redo` | false | **store `Redo`** (unified stack) |
| `app.command` | — | ui_state `ShowCommand` |
| `app.palette` | — | ui_state `ShowPalette` |
| `app.search` | — | ui_state `ShowSearch` |
| `app.dismiss` | — | ui_state `Dismiss` |
| `settings.keymap.vim` | — | ui_state `SetKeymapMode{vim}` |
| `settings.keymap.cua` | — | ui_state `SetKeymapMode{cua}` |
| `settings.keymap.emacs` | — | ui_state `SetKeymapMode{emacs}` |
| `drag.start` | false | ui_state `DragStart` |
| `drag.cancel` | false | ui_state `DragCancel` |
| `drag.complete` | false | ui_state `DragComplete` |

## Tallies
- Plugins: 7; source YAMLs: 12; commands: 3+5+4+17+8+10+15 = **62**
- Backend servers: `kanban`, `views`, `ui_state`, `window`, `app`, `store` (+ `commands`)

## Acceptance Criteria
- [ ] `plugins.yaml` has all 7 plugins, 62 commands, full metadata, `ensure_services`, per-command `backend`
- [ ] `baseline/mod.rs` loads it into typed structs
- [ ] Self-check test: tallies (7/12/62), id uniqueness, per-plugin counts, every `backend` server in the known set {kanban,views,ui_state,window,app,store}
- [ ] Drift test: source-YAML command-id set == catalog command-id set

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/baseline/catalog_self_check.rs`
- [ ] `crates/swissarmyhammer-command-service/tests/baseline/yaml_vs_catalog.rs`
- [ ] `cargo test -p swissarmyhammer-command-service --test baseline` passes

## Workflow
- Use `/tdd` — self-check + drift tests first.

Gates every plugin-port task. Each plugin port also depends on its backend server task(s) per the ensureServices column.