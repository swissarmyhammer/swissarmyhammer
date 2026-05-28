---
assignees:
- claude-code
depends_on:
- 01KS36WW3Q3N8518ZZJR431E7K
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc980
project: builtin-commands
title: 'Frontend: palette + hotkey + menu wiring via `list command`'
---
## What

Rewire the three command consumers in the frontend to read from `list command` (with `notifications/commands/changed` subscription) instead of a static, Rust-loaded list. Behavior must match today exactly.

Files:
- `apps/kanban-app/ui/src/components/CommandPalette.tsx` — read commands via `useCommandList({ scope: currentScope })`. The palette's fuzzy-match runs on the result. `available` is consulted by calling `available command` for each visible entry on open (debounced/concurrency-limited). Entries whose `available` returns false render grayed-out with the `reason` as tooltip.
- `apps/kanban-app/ui/src/hooks/useHotkeys.ts` (or wherever hotkey dispatch lives) — read `keys` from `useCommandList()` results; bind to the active keymap (vim/cua/emacs); on key press, look up the matching command id and dispatch via `useDispatchCommand`. Re-bind when keymap changes (which itself is a command — `settings.keymap.vim` etc.) or when the list changes.
- `apps/kanban-app/ui/src/components/ContextMenu.tsx` — read commands with `context_menu: true` matching the current scope; render in the right-click menu.
- `apps/kanban-app/ui/src/components/TabBar.tsx` — read commands with `tab_button` metadata; render as icon buttons.

Memory `metadata-driven-ui`: UI is an interpreter of metadata. No hardcoded command lists in React — every consumer reads from `useCommandList`.

Performance: the palette evaluates `available` for every visible command on open. Today this is sync against the Rust registry; new world calls `available command` per id. Mitigations:
- The Command service's latency budget (5ms warn / 50ms force-false) prevents runaway evaluation.
- Batch into one `tools/call` per palette open by reusing a top-level concurrency Promise.all over visible ids.
- Cache results until the next `commands/changed` notification or scope change.

## Acceptance Criteria
- [ ] Palette opens in <100ms with availability evaluated for every visible command
- [ ] Hotkey binding switches when keymap changes (vim/cua/emacs)
- [ ] Context menu shows the same items as today
- [ ] Tab button bar renders identical icon set
- [ ] No frontend code path hardcodes a command id list — all four consumers read from `useCommandList`

## Tests
- [ ] `apps/kanban-app/ui/src/components/CommandPalette.test.tsx` — palette with 20 mock commands; assert all 20 render; assert grayed-out reflects `available: false`; assert reason tooltip
- [ ] `apps/kanban-app/ui/src/hooks/useHotkeys.test.tsx` — vim keymap active; press `x` → `task.untag` dispatched; switch to cua keymap; press `Delete` → same command dispatched
- [ ] `apps/kanban-app/ui/src/components/ContextMenu.test.tsx` — right-click a task; assert only `context_menu: true` task-scoped commands appear
- [ ] `npm test --prefix apps/kanban-app/ui` passes

## Workflow
- Use `/tdd` — write the palette test first with the fuzzy-match + availability assertions; that exercises the most logic.