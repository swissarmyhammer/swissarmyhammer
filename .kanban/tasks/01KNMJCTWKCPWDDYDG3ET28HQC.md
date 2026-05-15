---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffda80
title: 'Perspective filter formula bar: move filter out of popover into right-side formula bar'
---
## What

Replace the filter popover with an Excel-style formula bar embedded in the right side of the perspective tab bar. Currently the filter lives in a `<Popover>` triggered by a filter button adjacent to the active tab. The new design makes the filter always visible in the remaining space to the right of the tabs.

### Target layout

```
┌─────────────────────────────────────────────────────────────────────┐
│ [tab1] [🔍][⊞]  [tab2]  [+]  │  🔍  [filter formula bar........]  │
│  ←── tabs (overflow-x-auto) ──→  ←── formula bar (flex-1) ────────→ │
└─────────────────────────────────────────────────────────────────────┘
```

- **Left region** (`overflow-x-auto`): all perspective tabs + "+" button, same as now
- **Right region** (`flex-1`): filter formula bar — always visible when an active perspective exists
- The filter `🔍` icon button on the active tab **focuses** the formula bar CM6 editor (does not open a popover)
- When no filter is set, the formula bar shows placeholder text: `"Filter… e.g. #bug @alice"`
- When a filter is active, the formula bar shows the current filter expression

### Files to modify

**`perspective-tab-bar.tsx`** — primary changes:
1. Restructure the outer `PerspectiveTabBar` div from a single flat flex row to two regions:
   ```tsx
   <div className="flex items-center border-b bg-muted/20 px-1 h-8 shrink-0">
     {/* Left: scrollable tabs */}
     <div className="flex items-center gap-0.5 overflow-x-auto shrink-0 max-w-[60%]">
       {tabs}
       <AddPerspectiveButton />
     </div>
     {/* Right: formula bar — separator + CM6 editor */}
     {activePerspective && (
       <FilterFormulaBar
         filter={activePerspective.filter}
         perspectiveId={activePerspective.id}
         editorRef={filterEditorRef}
       />
     )}
   </div>
   ```
2. Remove `FilterPopoverButton` popover — replace with a plain focus-trigger button that calls `filterEditorRef.current?.focus()`
3. Remove `filterOpen` / `setFilterOpen` state from `PerspectiveTab`
4. Add `filterEditorRef` (a `RefObject<{ focus(): void }>`) threaded from `PerspectiveTabBar` → `PerspectiveTab` → filter button

**New `FilterFormulaBar` component** (can live in `perspective-tab-bar.tsx` or a new file):
- `flex items-center gap-2 flex-1 min-w-0 border-l pl-2 ml-1`
- Filter icon (lucide `Filter`, size 14, `text-muted-foreground`) on the left
- CM6 `FilterEditor` content area (no border/box), `flex-1`, `text-sm`
- Placeholder text `"Filter… e.g. #bug @alice"` when empty
- Exposes a `focus()` method via `useImperativeHandle` or passes ref down to the CM6 editor

**`filter-editor.tsx`** — minor changes:
- Remove the `w-80` wrapper, `rounded-md border` box, and "Filter Expression" header — these were popover artifacts
- The component now renders its CM6 editor content inline (icon + editor, no container box)
- Keep all hook logic, validation, dispatch unchanged
- Keep `data-testid="filter-editor"`
- `onClose` prop becomes optional (no longer needed since there's no popover to close)

### What NOT to change
- Group popover (`GroupPopoverButton`) — leave as-is
- All filter dispatch/validation/command logic
- `StableCodeMirror` component
- Any backend/API

## Acceptance Criteria
- [ ] Filter formula bar is always visible to the right of the tabs when an active perspective exists
- [ ] Formula bar shows placeholder `"Filter… e.g. #bug @alice"` when filter is empty
- [ ] Formula bar shows current filter expression when filter is active
- [ ] Clicking the filter button (🔍 on the active tab) focuses the formula bar CM6 editor
- [ ] No popover opens when clicking the filter button
- [ ] Filter icon on active tab is highlighted (primary color, filled) when a filter is active
- [ ] Group popover is unchanged
- [ ] Typing in the formula bar updates the filter (same command dispatch as before)
- [ ] Clear functionality still works

## Tests
- [ ] `perspective-tab-bar.test.tsx` — formula bar renders when a perspective is active
- [ ] `perspective-tab-bar.test.tsx` — placeholder text visible when filter is empty
- [ ] `perspective-tab-bar.test.tsx` — filter button click focuses the formula bar
- [ ] `filter-editor.test.tsx` — all existing tests still pass
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.