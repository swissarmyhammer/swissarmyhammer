# Plan 4 — Builtin Commands (the commands)

**Kanban project:** `builtin-commands` · **Tier 2** · **Depends on:**
`command-service` (engine + SDK helpers) and `command-backends` (the servers
each command calls).

The 62 builtin commands as 7 TypeScript plugins, plus the locked catalog and
the frontend command-dispatch wiring. "What we actually do."

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS3BK37R2P0RYSYSXGZTS0Z3` | Plugin catalog: enumerate every builtin plugin + lock cut-over baseline | op-structs | `tests/baseline/plugins.yaml` for all 7 plugins / 62 commands w/ ensureServices + per-command backend; self-check (7/12/62) + YAML-drift tests. Gates every port. |
| `01KS36RBS1KB6T21ENB9X7H14M` | Builtin plugin: task commands (port task.yaml) | sdk-helpers, catalog | `task-commands` (3): move/untag/doThisNext → `kanban`; metadata fidelity. |
| `01KS36RT7F7WZMNBCHER0HRGKM` | Builtin plugin: column/attachment/tag/view (4 small YAMLs) | sdk-helpers, catalog, kanban, window, views | `kanban-misc-commands` (5): column.reorder + tag.update → kanban, attachment open/reveal → window, view.set → views. |
| `01KS36SEXMBGZJTWJX0ZQQKP8V` | Builtin plugin: file/board commands (port file.yaml) | sdk-helpers, catalog, window board-lifecycle (`01KS612DV4`) | `file-commands` (4): switch/close/new/openBoard → window board lifecycle. |
| `01KS36SWFYJRPQHD073FTRZYAE` | Builtin plugin: perspective commands (perspective.yaml, 17) | sdk-helpers, catalog, views | `perspective-commands` (17) → views; subfiles filter/group/sort/nav/lifecycle. |
| `01KS36TCNMSDGSQBZP3NKY6YK7` | Builtin plugin: entity + clipboard (entity.yaml) | sdk-helpers, catalog, **entity CRUD** (`01KS5EAD57`) + **entity clipboard** (`01KS614S1`) | `entity-commands` (8): CRUD + archive + clipboard → **`entity` server**; `from: target`. |
| `01KS36TSWE3NR5MFQTY99JX5TB` | Builtin plugin: ui commands (ui.yaml, 10 incl. window.new) | sdk-helpers, catalog, ui_state, window, **focus** | `ui-commands` (10) → ui_state; `ui.setFocus` → **focus** (spatial-nav); window.new → window. |
| `01KS36V80DXK2BFDDSHSWP131W` | Builtin plugin: app + settings + drag (app/settings/drag.yaml) | sdk-helpers, catalog, app, ui_state, store, entity search (`01KS61511W`) | `app-shell-commands` (15): quit/about/help → app, undo/redo → store, command/palette/search/dismiss + keymap + drag → ui_state (search QUERY → `entity` Search). |
| `01KS36WW3Q3N8518ZZJR431E7K` | Frontend: rewrite `useDispatchCommand` to route through the Command service | engine bootstrap | Hook calls `execute command` via MCP; signature unchanged; `useCommandList` subscribes to `commands/changed`. |
| `01KS36XGKCQ36QM7P6MH3FHMBJ` | Frontend: palette + hotkey + menu wiring via `list command` | useDispatchCommand | Palette/hotkey/context-menu/tab-button read `list command` + `available command`; no hardcoded command lists; keymap-aware. |

## Key decisions baked in

- **Plugins on disk** under `builtin/plugins/` — the host has no command-specific
  code; built-ins ride the same path as user plugins.
- Every command's metadata (keys/scope/params/undoable/context_menu/tab_button)
  ported 1:1 from YAML; the catalog drift test guards against silent loss.
- `load()` convention: `ensureServices(this, [...])` then `registerCommands(...)`.
- Frontend command dispatch keeps `useDispatchCommand(id)` signature; only the
  backend changes (Rust registry → Command MCP).

## Cross-check

`kanban list tasks --filter '$builtin-commands'` → expect exactly these 10 tasks.
