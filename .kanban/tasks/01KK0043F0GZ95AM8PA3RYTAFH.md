---
position_column: done
position_ordinal: ffffb380
title: Delete old command infrastructure and dead React state code
---
Clean cut removal of all superseded code once the new system is working.

## Scope

- Delete `execute_command` dispatcher in `swissarmyhammer-kanban-app/src/commands.rs` (the old match-on-string router)
- Delete old individual Tauri query commands that are now served by dispatch or cache: `get_board_data`, `list_entities`, `get_entity`, `get_keymap_mode`, `set_keymap_mode`, `list_views` (replaced by events + cache)
- Delete React files:
  - `lib/undo-stack.ts` and `lib/undo-context.tsx` — undo is Rust-only
  - `lib/app-mode-context.tsx` — mode state comes from Rust events
  - `lib/keymap-context.tsx` — keymap comes from Rust events
  - `lib/field-update-context.tsx` — no more centralized refresh callback
  - `lib/views-context.tsx` internal state (may keep as event listener shell)
- Delete React code:
  - `CommandDef.execute` and `CommandDef.rustCommand` fields
  - `dispatchCommand` function (replaced by invoke to Rust)
  - `task-defaults.ts` — moved to Rust
  - `computeOrdinal` / `midpointOrdinal` in `board-view.tsx` — moved to Rust
- Delete `ViewCommand` / `ViewCommandKeys` types from `swissarmyhammer-views` (commands crate owns this now)
- Remove inline command definitions from `builtin/views/board.yaml` (replaced by command ID references)
- Clean up unused imports, dead test files

## Testing

- Test: `cargo build` succeeds with no warnings about dead code
- Test: `npm run build` succeeds with no TypeScript errors
- Test: `cargo test` passes — no tests reference deleted code
- Test: `npm test` passes — no tests reference deleted contexts/hooks
- Test: grep for "execute_command" in React codebase returns zero hits
- Test: grep for "UndoStack" in React codebase returns zero hits
- Test: grep for "AppModeProvider" returns zero hits
- Test: full app smoke test — board loads, task CRUD works, undo works, inspector works, DnD works