---
assignees:
- claude-code
depends_on:
- 01KNC7NQA00AZNR027JPJTQKWD
position_column: todo
position_ordinal: '8880'
position_swimlane: container-refactor
title: Refactor NavBar to read from container contexts
---
## What

NavBar currently receives `board`, `openBoards`, `activeBoardPath`, and `onSwitchBoard` as props drilled from App.tsx. After the container extraction, these values live in WindowContainer and BoardContainer contexts. Refactor NavBar to be a pure presenter that reads from those contexts instead of taking props.

**Files to modify:**
- `kanban-app/ui/src/components/nav-bar.tsx` — remove props, use `useWindowContext()` or `useBoardContext()` hooks to get board data
- Whichever container provides the board data context (WindowContainer or BoardContainer) — ensure the context is exported

**Current NavBar props (nav-bar.tsx:9-15):**
```typescript
interface NavBarProps {
  board: BoardData | null;
  openBoards: OpenBoard[];
  activeBoardPath?: string;
  onSwitchBoard: (path: string) => void;
}
```

**Target:** NavBar takes no props. Reads everything from context.

## Acceptance Criteria
- [ ] NavBar has no props (or minimal presentation-only props)
- [ ] Board selector still shows open boards and allows switching
- [ ] Board inspect button still works
- [ ] Percent complete field still renders
- [ ] Search button still works

## Tests
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: verify board selector dropdown, board switching, inspect button