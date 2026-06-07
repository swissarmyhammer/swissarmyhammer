---
depends_on:
- 01KTED6YMERJHTS7QDSTV5MZYG
- 01KTED7833AJJB5JPTZVNF42HN
- 01KTED7PFKRS6GMAQKVDCQA07V
- 01KTED80H7GNF6YJJTE8MQP7CQ
- 01KTED8MS8917AJCDAVHKSZHK7
- 01KTED8XDX4728QR4WT9EZ0WRF
position_column: todo
position_ordinal: dc80
project: ui-command-cleanup
title: Card I — Delete STATIC_GLOBAL_COMMANDS + buildAiCommands dup; remove dead CommandDef.execute fast-path
---
## What
FINAL REMOVAL once no scope-level command executes remain (all prior cards done).

In `apps/kanban-app/ui/src/components/app-shell.tsx`:
- Delete `STATIC_GLOBAL_COMMANDS` (line ~187): app.command, app.palette, app.undo/redo/dismiss/search/help/quit, settings.keymap.{vim,cua,emacs}, app.resetWindows, file.{newBoard,openBoard,closeBoard}, window.new, app.about — all pure metadata already duplicated in `builtin/plugins/app-shell-commands`, `builtin/plugins/file-commands`, and the window/ui plugins. Verify each id has a plugin equivalent BEFORE deleting; any without one gets added to the appropriate plugin in this card.
- Delete `buildAiCommands` (line ~584) — duplicates `builtin/plugins/ai-commands`. The `ai.cancel` availability gate that `buildAiCommands` computed from `aiStreaming()` must be re-expressed: either as a plugin `available` callback or kept frontend-side per the ai/commands.ts note — preserve the behavior, delete the duplicate definition.

In `apps/kanban-app/ui/src/lib/command-scope.tsx`:
- Remove the now-dead `resolveCommand` execute fast-path and the `CommandDef.execute` field — once Cards C/D/E/F/G/H moved every scope exec to the handler bus or backend, no CommandDef carries a client `execute`. Remove the field and the fast-path that invoked it. Keep `CommandScopeProvider`/`resolveCommand` (resolution)/`useDispatchCommand`/`runBackendDispatch` and the bus lookup added in Card B.

KEEP (presentation): `use-command-list.ts`, `command-palette.tsx`, `lib/context-menu.ts`, the KeybindingHandler/executeCommand/menu-command+context-menu listeners, keybindings.ts normalize/createKeyHandler/extractScopeBindings/extractKeymapBindings.

## Acceptance Criteria
- [ ] `STATIC_GLOBAL_COMMANDS` and `buildAiCommands` are deleted from app-shell.tsx; every id they carried is plugin-defined.
- [ ] `CommandDef.execute` field and the `resolveCommand` execute fast-path are removed from command-scope.tsx; no CommandDef carries a client execute.
- [ ] `ai.cancel` availability behavior preserved.
- [ ] Presentation-layer command reading/dispatch still works end to end.

## Tests
- [ ] UI: a test (e.g. `app-shell.test.tsx`) asserting no client-built global CommandDef list exists and the global commands resolve from the service catalogue.
- [ ] UI: command-scope test asserting `CommandDef` has no `execute` field and dispatch goes via bus-or-backend only.
- [ ] UI: AI cancel-availability test still green.
- [ ] Full relevant vitest suite green; `cargo test` for any touched Rust command-service drift/baseline tests green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.