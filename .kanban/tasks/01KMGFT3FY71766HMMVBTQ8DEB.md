---
assignees:
- claude-code
depends_on:
- 01KMGFSHVGCZZPE563P6Y6R3TB
position_column: done
position_ordinal: fffffffffffffe80
title: Wire board navigation commands into BoardView scope
---
## What

Add keyboard navigation commands to `board-view.tsx` by wiring `useBoardNav` into a `CommandDef[]` array and registering it via `CommandScopeProvider`, following the exact same pattern as `grid-view.tsx` lines 95–212.

### Changes to `kanban-app/ui/src/components/board-view.tsx`

1. Import and call `useBoardNav({ columnCount, cardCounts })` where `cardCounts` is derived from `baseLayout`
2. Define `boardNavCommands: CommandDef[]` with vim/cua keybindings:

| Vim | CUA | Command ID | Action |
|-----|-----|------------|--------|
| `h` | `ArrowLeft` | `board.moveLeft` | Move to previous column |
| `l` | `ArrowRight` | `board.moveRight` | Move to next column |
| `k` | `ArrowUp` | `board.moveUp` | Move to previous card in column |
| `j` | `ArrowDown` | `board.moveDown` | Move to next card in column |
| `g g` | `Home` | `board.firstColumn` | First column (already in SEQUENCE_TABLES) |
| `G` | `End` | `board.lastColumn` | Last column |
| `Enter` | `Enter` | `board.inspect` | Open inspector for focused card |
| `o` | `Mod+Enter` | `board.newTask` | Add task to current column |

3. Wrap the existing `FocusScope` content with `<CommandScopeProvider commands={boardNavCommands}>` (or merge into the existing FocusScope's commands)
4. Add a `BoardFocusBridge` component (like `GridFocusBridge`) to register the scope in the entity focus system so the keybinding handler picks it up

### Files to modify

- **Modify**: `kanban-app/ui/src/components/board-view.tsx`
- **Modify**: `kanban-app/ui/src/lib/keybindings.ts` — add `G` → `board.lastColumn` to SEQUENCE_TABLES or BINDING_TABLES as appropriate

## Acceptance Criteria

- [ ] `h`/`l` or arrows move between columns
- [ ] `j`/`k` or arrows move between cards within a column  
- [ ] `gg`/`Home` jumps to first column, `G`/`End` to last
- [ ] `Enter` on a card opens inspector (calls `inspectEntity`)
- [ ] `o`/`Mod+Enter` adds a new task to the current column
- [ ] Commands appear in command palette when board is focused
- [ ] Keybindings don't fire when in an editor (existing skip logic handles this)

## Tests

- [ ] `kanban-app/ui/src/components/__tests__/board-view.test.tsx` — test that board commands are registered in scope
- [ ] `pnpm vitest run` passes