---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: ab80
title: Add visible "+" button to grid view for adding new entities
---
## What

The grid view currently has no visible UI for adding entities — it's keyboard-only (`o`/`O` in vim, `Mod+Enter` in CUA). Add a visible "+" button that dispatches the same `${entityType}.add` command.

**Files to modify:**

1. `kanban-app/ui/src/components/grid-view.tsx` — add a "+" button below the `DataTable` or in the `GridStatusBar`. The button should:
   - Use the same plain `<button>` + `Tooltip` pattern from `column-view.tsx` (the board's add-task button)
   - Use `Plus` icon from `lucide-react` at `h-4 w-4`
   - Style: `text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted` (matching board pattern)
   - Label: `"Add ${entityType}"` with first letter capitalized (e.g. "Add Task", "Add Tag")
   - On click: dispatch `${entityType}.add` with `{ title: \`New ${entityType}\` }` (same as `grid.newBelow`)
   - Place it in a thin bar below the table: `<div className="flex items-center px-2 py-1 border-t border-border">` with the button left-aligned

2. `kanban-app/ui/src/components/grid-view.tsx` — extract a small capitalize helper (inline or from `entity-commands.ts`'s `resolveCommandName` pattern): `entityType.charAt(0).toUpperCase() + entityType.slice(1)`

**UI reference:** The board view's add button in `column-view.tsx`:
```tsx
<button className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors">
  <Plus className="h-4 w-4" />
</button>
```

## Acceptance Criteria
- [ ] Grid view shows a visible "+" button (with tooltip "Add Task" / "Add Tag" / "Add Project" depending on entity type)
- [ ] Clicking the button creates a new entity of the correct type
- [ ] Button style matches the board view's add-task button pattern
- [ ] Button is always visible (not hidden behind keyboard shortcuts)
- [ ] Keyboard shortcuts (`o`, `O`, `Mod+Enter`) still work

## Tests
- [ ] Add test in `kanban-app/ui/src/components/grid-view.test.tsx` — verify "+" button renders with correct aria-label for entity type
- [ ] Add test — clicking "+" button dispatches `${entityType}.add`
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/grid-view` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.