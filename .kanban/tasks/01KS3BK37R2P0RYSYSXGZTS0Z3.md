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
- `crates/swissarmyhammer-command-service/tests/baseline/plugins.yaml`
- `crates/swissarmyhammer-command-service/tests/baseline/mod.rs` — loader → typed `PluginSpec`.

## Backend services (where the work happens)

All store-backed servers share ONE `Arc<StoreContext>` (single undo substrate). `EntityContext` is the entity **kernel**; both `entity` and `kanban` are MCP faces over it.

| Server | Status | Backs |
| ------ | ------ | ----- |
| `commands` | built here | the registry every plugin registers into |
| `store` | NEW (store-service) | undo/redo (unified stack), transaction grouping, per-store history |
| `entity` | NEW (entity-service) | **generic** MCP face over the kernel: get/list/add/update/delete + archive/unarchive + clipboard cut/copy/paste + **search**, for any type |
| `kanban` | EXISTS (unchanged surface) | domain facade over the SAME kernel: keeps ALL its ops — `add/update/delete/get` task/column/tag/project/actor, `move task`, `next/complete task`, `assign`, `tag/untag`, board lifecycle. Generic CRUD delegates to the kernel internally |
| `views` | NEW | perspective.* + view.set |
| `ui_state` | NEW | ui.* (minus setFocus) + settings.keymap.* + drag.* + app.command/palette/search/dismiss |
| `window` | NEW | window.new + file.* board lifecycle + attachment.open/reveal |
| `app` | NEW | app.quit/about/help only |
| `focus` | NEW (spatial-nav project) | ui.setFocus + spatial nav (SpatialRegistry/SpatialState) |

Notes: **kanban does not lose operations** — `entity` is an additive generic face over the shared `EntityContext` kernel; kanban keeps its full domain surface and passes generic CRUD through to the kernel. **Search is an entity capability** (on `entity`, not a separate server). `undoable` is declarative metadata, not the undo gate.

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
`file.switchBoard/closeBoard/newBoard/openBoard` (undoable:false) → window board lifecycle.

### 4. `perspective-commands` — 17 — ensureServices `[commands, views]` — perspective.yaml
All → `views`. subfiles `commands/{filter,group,sort,nav,lifecycle}.ts`.

### 5. `entity-commands` — 8 — ensureServices `[commands, entity]` — entity.yaml
Cross-cutting, `from: target`; dynamic `entity.add:type` synthesized client-side. ALL → `entity` server (generic, type-agnostic).
| id | undoable | ctx_menu | backend |
|----|---------:|:--------:|---------|
| `entity.add` | true | — | entity `AddEntity` |
| `entity.update_field` | true | — | entity `UpdateField` |
| `entity.delete` | true | ✓ | entity `DeleteEntity` |
| `entity.archive` | true | ✓ | entity `ArchiveEntity` |
| `entity.unarchive` | true | ✓ | entity `UnarchiveEntity` |
| `entity.cut` | true | ✓ | entity `Cut` |
| `entity.copy` | false | ✓ | entity `Copy` |
| `entity.paste` | true | ✓ | entity `Paste` |

### 6. `ui-commands` — 10 — ensureServices `[commands, ui_state, window, focus]` — ui.yaml
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
| `ui.setFocus` | — | false | — | **focus** (spatial-nav project) |
| `window.new` | — | — | — | window `OpenNewWindow` |

### 7. `app-shell-commands` — 15 — ensureServices `[commands, app, ui_state, store]` — app/settings/drag.yaml
| id | undoable | backend |
|----|---------:|---------|
| `app.about`/`app.help`/`app.quit` | — | app |
| `app.undo`/`app.redo` | false | store `Undo`/`Redo` |
| `app.command`/`app.palette`/`app.search`/`app.dismiss` | — | ui_state (UI toggles; search QUERY uses `entity` `Search`) |
| `settings.keymap.{vim,cua,emacs}` | — | ui_state `SetKeymapMode` |
| `drag.{start,cancel,complete}` | false | ui_state `Drag*` |

## Tallies
- Plugins: 7; source YAMLs: 12; commands: 3+5+4+17+8+10+15 = **62**
- Backend servers referenced: `commands`, `store`, `entity`, `kanban`, `views`, `ui_state`, `window`, `app`, `focus`

## Acceptance Criteria
- [ ] `plugins.yaml` has all 7 plugins, 62 commands, full metadata, `ensure_services`, per-command `backend`
- [ ] Self-check: tallies (7/12/62), id uniqueness, every `backend` in the known server set
- [ ] Drift test: source-YAML command-id set == catalog command-id set

## Tests
- [ ] `tests/baseline/catalog_self_check.rs`, `tests/baseline/yaml_vs_catalog.rs`
- [ ] `cargo test -p swissarmyhammer-command-service --test baseline` passes

## Workflow
- Use `/tdd` — self-check + drift tests first.

Gates every plugin-port task.