---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
title: Restyle FilterEditor as inline field row (icon + editor, no border box)
---
## What

Restyle the FilterEditor component to match the inspector field-row pattern: a filter icon on the left and the CM6 editor to its right, inline like a regular field. Remove the compressed popover-style `w-80` box, the `rounded-md border` wrapper, and the redundant "Filter Expression" label header.

### Current layout (popover-style)
```
┌─ Filter Expression ──── [Clear] ─┐
│ ┌─────────────────────────────┐  │
│ │ CM6 editor (bordered)       │  │
│ └─────────────────────────────┘  │
│ #tag @user ... Enter/Esc         │
└──────────────────────────────────┘
```

### Target layout (field-row style)
```
🔍  CM6 editor (no border, flex-1)         [x Clear]
    #tag @user ^ref, &&/and ...
```

### Pattern to follow
Inspector field rows (`entity-inspector.tsx` FieldRow, lines 328-351) use:
- `flex items-start gap-2` container
- Lucide icon (`size={14}`, `text-muted-foreground`, `mt-0.5 shrink-0`)
- Content area with `flex-1 min-w-0`

### Files to modify
- `kanban-app/ui/src/components/filter-editor.tsx` — `FilterEditor` JSX (lines 238-280):
  - Replace the `w-80` wrapper with a field-row-style flex container
  - Add a `Filter` (funnel) icon from lucide-react on the left, matching inspector icon styling
  - Remove the `rounded-md border bg-background` div around the CM6 editor
  - Move the Clear button inline to the right of the editor (or at the end of the row)
  - Keep error display below (red text, no border change needed on the editor itself)
  - Keep help text below but more compact
  - Set `autocompletion: true` in `BASIC_SETUP` since the filter editor now has autocomplete wired

### What NOT to do
- Do NOT change any hook logic, validation, or command dispatch — only JSX and CSS classes
- Do NOT change the StableCodeMirror component
- Do NOT remove the `data-testid="filter-editor"` attribute

## Acceptance Criteria
- [ ] Filter editor renders as: icon (left) + CM6 editor (right, no border), matching field-row layout
- [ ] Filter icon uses lucide `Filter` or `SlidersHorizontal` icon at 14px, `text-muted-foreground`
- [ ] No `rounded-md border` wrapper around the CM6 editor
- [ ] Clear button is inline, visible when filter is non-empty
- [ ] Error text still shows below on invalid input
- [ ] Help text still visible but compact (single line)
- [ ] All existing filter-editor tests pass
- [ ] `data-testid=\"filter-editor\"` preserved

## Tests
- [ ] `kanban-app/ui/src/components/filter-editor.test.tsx` — all 6 existing tests pass
- [ ] `cd kanban-app/ui && npx vitest run src/components/filter-editor.test.tsx` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.