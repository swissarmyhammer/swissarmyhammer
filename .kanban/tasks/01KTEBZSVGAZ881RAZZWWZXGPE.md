---
assignees:
- claude-code
depends_on:
- 01KTED9JYGWM815K2X41N4QDBY
position_column: todo
position_ordinal: d280
project: ui-command-cleanup
title: Rename all ui.* commands to app.* — eliminate the "ui" command namespace
---
## What
Decision (user): **there is no `ui.*` command namespace. Every former `ui.*` command is renamed to `app.*`.** UI-surface commands are app commands. The command-id namespace is independent of which MCP server backs the command — renaming the id does NOT change which server answers it.

## IMPORTANT — fold into the ui-command-cleanup project (owner decision)
The `ui-command-cleanup` project (Cards A,C,D,E,F,G,H) MOVES many of these commands into plugins. Per "fold rename into each move", those moves adopt the `app.*` name AT MOVE TIME — so the moved commands never carry a `ui.*` id. THIS card only mops up the `ui.*` ids that the cleanup project does NOT move. Before working this card, check what the cleanup has already renamed:
- `app.palette.open` — done by Card A (01KTCQFH7AEQDZD0QETSMCMGP0).
- `app.inspect` / `app.inspector.close` (+ close_all/set_width) — done by Card G (01KTED8MS8917AJCDAVHKSZHK7).
- `app.entity.startRename` — done where that command is moved.
- `app.ai-panel.*` editor drill-ins — done by Card E (01KTED7PFKRS6GMAQKVDCQA07V).
This card = the remainder + the repo-wide guard test (no id starts with `ui.`).

## Rename map (uniform `app.*` — user: "all called app")
| Current id | New id | Backing MCP server (UNCHANGED) |
|---|---|---|
| `ui.inspect` | `app.inspect` | `ui_state` |
| `ui.inspector.close` | `app.inspector.close` | `ui_state` |
| `ui.inspector.close_all` | `app.inspector.close_all` | `ui_state` |
| `ui.inspector.set_width` | `app.inspector.set_width` | `ui_state` |
| `ui.palette.open` | `app.palette.open` | `ui_state` |
| `ui.palette.close` | `app.palette.close` | `ui_state` |
| `ui.mode.set` | `app.mode.set` | `ui_state` |
| `ui.entity.startRename` | `app.entity.startRename` | `ui_state` |
| `ui.setFocus` | `app.setFocus` | **`focus`** (id changes to app.*, routing stays focus) |

`window.new` (also in this plugin) is NOT a `ui.*` command — it keeps `window.new` and its `window` server routing. Out of scope for the rename; only its plugin home may move.

## Approach
- Rename every remaining `ui.*` id to `app.*` and fold the `ui-commands` plugin registrations into `app-shell-commands` (single app-command plugin).
- Each command keeps its existing MCP-server call verbatim — pure namespace/registration move, zero behavior change. `app.setFocus` still calls the `focus` server.

## Blast radius (update together)
- `builtin/plugins/ui-commands/index.ts` (+ `context.ts`) → merged into `builtin/plugins/app-shell-commands/*`
- Frontend keymap / scope references to `ui.*` ids in `apps/kanban-app/ui/src` (keybindings, scope-claim lookups, menu/context-menu wiring)
- `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs` + `builtin_app_shell_commands_e2e.rs`
- Repo-wide grep for old ids (`ui\.inspect`, `ui\.inspector`, `ui\.palette`, `ui\.mode`, `ui\.entity`, `ui\.setFocus`)

## Acceptance Criteria
- [ ] No registered command id begins with `ui.` — all are `app.*`.
- [ ] Each renamed command routes to the SAME MCP server as before: inspector/palette/mode/rename/inspect → `ui_state`; `app.setFocus` → `focus`.
- [ ] All keybindings, menu placements, and context-menu entries resolve under the new `app.*` ids — no dangling `ui.*` reference anywhere.

## Tests
- [ ] Update `builtin_ui_commands_e2e.rs` / `builtin_app_shell_commands_e2e.rs` to assert the renamed `app.*` id set and per-command server routing.
- [ ] Guard test: assert NO registered command id starts with `ui.` (regression lock).
- [ ] Frontend keymap test: former `ui.*` shortcuts resolve under the new `app.*` ids.
- [ ] Tests fail before the rename, pass after.

## Workflow
- Use `/tdd` — write the no-`ui.`-prefix guard test first, then migrate.

## Related
- ui-command-cleanup project (folds most renames in at move time). Palette consolidation `01KTCRQ6KJ67FJWYEZFQ6J7R13`.