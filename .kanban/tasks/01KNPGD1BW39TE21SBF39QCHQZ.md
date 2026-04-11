---
assignees:
- claude-code
depends_on:
- 01KNPGC2K0ETKC255A29PA4V0D
position_column: todo
position_ordinal: aa80
title: Add visible "+" button to grid view dispatching entity.add
---
## What

The grid view has no visible UI for adding entities — only keyboard shortcuts. Add a "+" button and update grid commands to use the generic `entity.add:{entityType}` mechanism.

**File to modify:** `kanban-app/ui/src/components/grid-view.tsx`

### 1. Add "+" button below the DataTable

In the `GridView` return JSX, after `<DataTable ... />`, add a thin action bar:

```tsx
<div className=\"flex items-center px-2 py-1 border-t border-border\">\n  <Tooltip>\n    <TooltipTrigger asChild>\n      <button\n        type=\"button\"\n        aria-label={`Add ${entityType.charAt(0).toUpperCase() + entityType.slice(1)}`}\n        className=\"p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors\"\n        onClick={() => {\n          dispatch(`entity.add:${entityType}`, {\n            args: { title: `New ${entityType}` },\n          }).catch((err) => console.error(\"Failed to add entity:\", err));\n        }}\n      >\n        <Plus className=\"h-4 w-4\" />\n      </button>\n    </TooltipTrigger>\n    <TooltipContent>\n      {`Add ${entityType.charAt(0).toUpperCase() + entityType.slice(1)}`}\n    </TooltipContent>\n  </Tooltip>\n</div>
```

This matches the board view's add button pattern from `column-view.tsx` — plain `<button>`, `Plus` icon, tooltip, muted styling.

### 2. Update `buildGridEditCommands` to use `entity.add`

Change `grid.newBelow` and `grid.newAbove` to dispatch `entity.add:${entityType}` instead of `${entityType}.add`:

```tsx
execute: () => {
  dispatch(`entity.add:${entityType}`, {
    args: { title: `New ${entityType}` },
  }).catch(...);\n}
```

### 3. Add `Plus` to imports

Add `Plus` to the lucide-react imports if not already imported.

## Acceptance Criteria
- [ ] Grid view shows a visible "+" button below the table
- [ ] Tooltip shows "Add Task" / "Add Tag" / "Add Project" based on entity type
- [ ] Clicking "+" creates a new entity of the correct type
- [ ] Button style matches board view's add-task button (muted, Plus icon, hover states)
- [ ] Keyboard shortcuts (`o`, `O`, `Mod+Enter`) still work via updated `entity.add:*` dispatch
- [ ] Works on empty grids (no rows)

## Tests
- [ ] Add test in `kanban-app/ui/src/components/grid-view.test.tsx` — verify "+" button renders with correct aria-label for entity type
- [ ] Add test — clicking "+" button dispatches `entity.add:{entityType}`
- [ ] Existing grid-view tests still pass
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/grid-view` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.