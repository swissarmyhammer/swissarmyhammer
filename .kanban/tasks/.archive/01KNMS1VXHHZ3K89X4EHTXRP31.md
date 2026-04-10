---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
title: Restyle filter input to match inspector field pattern (icon + borderless CM6)
---
## What

The filter editor's CM6 input in `filter-editor.tsx` is wrapped in a `rounded-md border bg-background border-input` div (line ~256), creating a bordered box that's visually inconsistent with how other CM6 fields render throughout the app.

Other CM6 editors (TextEditor, MultiSelectEditor) render borderless with just `className="text-sm"`, sitting inline in their containers. Inspector fields use a `flex items-start gap-2` layout with a muted icon on the left.

Restyle the filter editor to match:

**Files to modify:**
- `kanban-app/ui/src/components/filter-editor.tsx` — remove the bordered wrapper div, add Filter icon inline, render CM6 borderless

**Specific changes in `filter-editor.tsx`:**
1. Remove the `<div className="rounded-md border bg-background border-input">` wrapper around `<StableCodeMirror>` (~line 256-259)
2. Add a `Filter` icon (from `lucide-react`, size 14) to the left of the CM6 editor using the inspector field layout pattern:
   ```tsx
   <div className="flex items-start gap-2">
     <span className="mt-0.5 shrink-0 text-muted-foreground">
       <Filter size={14} />
     </span>
     <div className="flex-1 min-w-0">
       <StableCodeMirror className="text-sm" ... />
     </div>
   </div>
   ```
3. The CM6 editor should have only `className="text-sm"` — no border classes, matching TextEditor and MultiSelectEditor patterns

**Reference patterns:**
- Inspector icon+content layout: `entity-inspector.tsx` lines 328-350
- Borderless CM6 field: `fields/text-editor.tsx` line 309
- CM6 theme (transparent bg, no outline): `lib/cm-keymap.ts` `shadcnTheme`

## Acceptance Criteria
- [ ] Filter input has no rounded border wrapper
- [ ] Filter icon (lucide `Filter`, size 14, muted-foreground) appears to the left of the CM6 input
- [ ] CM6 editor renders inline/borderless, matching TextEditor field style
- [ ] Error state border (border-destructive) still visually indicates errors — apply to the container or use a different indicator
- [ ] Filter editor still functions correctly inside its popover in perspective-tab-bar.tsx

## Tests
- [ ] Update `kanban-app/ui/src/components/filter-editor.test.tsx` — verify Filter icon renders in the component
- [ ] Existing filter-editor tests still pass (typing, clearing, error display)
- [ ] Run: `cd kanban-app/ui && npx vitest run src/components/filter-editor.test.tsx` — all tests pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.