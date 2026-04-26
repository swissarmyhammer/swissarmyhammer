---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffc980
title: Decompose BoardView and GridView into smaller components
---
## What

BoardView (~505 lines) and GridView (~385 lines) are large React components that combine state management, effects, handlers, and complex JSX rendering. They should be decomposed into smaller, focused components and hooks.

### Targets
- `kanban-app/ui/src/components/board-view.tsx` — BoardView (~505 lines)
- `kanban-app/ui/src/components/grid-view.tsx` — GridView (~385 lines), including `gridCommands` useMemo (~149 lines)

### Approach
- Extract custom hooks for drag-and-drop state, layout computation, and command definitions
- Extract sub-components for drag overlay, column rendering, and grid command setup
- Preserve existing test coverage — no behavioral changes

## Acceptance Criteria
- [ ] BoardView and GridView components are each under 50 lines
- [ ] All extracted hooks and sub-components are individually testable
- [ ] Existing tests pass without modification
- [ ] No behavioral changes