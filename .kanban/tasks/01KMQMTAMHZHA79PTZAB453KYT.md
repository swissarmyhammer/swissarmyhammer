---
assignees:
- claude-code
depends_on:
- 01KMQMSTA0PYVHXKWKGT4631PN
position_column: done
position_ordinal: ffffffffffffffbf80
title: Register global nav commands and fix sequence tables
---
## What

Register six universal navigation commands in app-shell's global CommandScopeProvider. These commands don't navigate themselves — they call `broadcastNavCommand` so registered `claimWhen` predicates can pull focus.

### Commands

| Command ID | Vim key | CUA key | Vim sequence |
|---|---|---|---|
| `nav.up` | `k` | `ArrowUp` | — |
| `nav.down` | `j` | `ArrowDown` | — |
| `nav.left` | `h` | `ArrowLeft` | — |
| `nav.right` | `l` | `ArrowRight` | — |
| `nav.first` | — | `Home` | `g g` |
| `nav.last` | `G` | `End` | — |

### Files to modify

- **`kanban-app/ui/src/components/app-shell.tsx`** — add nav commands to global `CommandScopeProvider`. Each command's `execute` calls `broadcastNavCommand(commandId)` from EntityFocusProvider context.
- **`kanban-app/ui/src/lib/keybindings.ts`** — change `SEQUENCE_TABLES` vim `g g` from `board.firstCard` to `nav.first`. This fixes the bug where `g g` was hardcoded to the board even when inspector was focused.
- **`kanban-app/ui/src/lib/keybindings.ts`** — remove `Shift+G` / `G` from BINDING_TABLES if present (it becomes a scope binding via nav.last's keys, not a global).

### Design note

These commands coexist with the existing `board.move*`, `grid.move*`, `inspector.move*` commands during migration. The old commands continue to work. As each view migrates to `claimWhen`, its old commands get removed. The global nav commands are always available.

## Acceptance Criteria

- [ ] Six `nav.*` commands registered in app-shell global scope
- [ ] Each command calls `broadcastNavCommand` — no direct focus manipulation
- [ ] `g g` sequence maps to `nav.first` (not `board.firstCard`)
- [ ] Old view-specific commands still work (coexistence during migration)
- [ ] `pnpm vitest run` passes

## Tests

- [ ] `kanban-app/ui/src/lib/keybindings.test.ts` or `command-scope.test.tsx` — `g g` sequence resolves to `nav.first`
- [ ] `kanban-app/ui/src/lib/entity-focus-context.test.tsx` — nav command broadcast triggers claim evaluation
- [ ] `pnpm vitest run` passes"