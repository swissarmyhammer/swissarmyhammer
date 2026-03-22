---
assignees:
- claude-code
depends_on:
- 01KMAQS9WS28VTKJZ4TWDRJG89
position_column: todo
position_ordinal: '8180'
title: EntityInspector visual focus ring and field indexing
---
## What

Wire `useInspectorNav` into `EntityInspector` (`kanban-app/ui/src/components/entity-inspector.tsx`).

The inspector currently renders fields via `FieldRow` with no concept of which field is "focused". This card adds:

1. **Flatten field list**: Build a single ordered array of all navigable fields (header + body + footer, excluding hidden). Pass total count to `useInspectorNav`.
2. **Focus ring**: The `FieldRow` at `focusedIndex` gets a visible focus indicator (e.g., `ring-2 ring-ring` or a left-border highlight). Use a `data-focused` attribute or a `focused` prop.
3. **Edit trigger**: When `useInspectorNav.mode === "edit"`, the focused `FieldRow` enters editing state automatically (currently controlled by `FieldRow`'s internal `editing` useState — connect the two).
4. **Scroll into view**: When focusedIndex changes, scroll the focused field row into view using `scrollIntoView({ block: "nearest" })`.

### Files to modify
- `kanban-app/ui/src/components/entity-inspector.tsx` — integrate hook, pass focused state to FieldRow
- `kanban-app/ui/src/components/entity-inspector.test.tsx` — add tests for focus rendering

## Acceptance Criteria
- [ ] Focused field has a visible focus ring/highlight
- [ ] Focus follows useInspectorNav.focusedIndex
- [ ] Editing a focused field syncs with useInspectorNav mode
- [ ] Focused field scrolls into view when off-screen

## Tests
- [ ] `kanban-app/ui/src/components/entity-inspector.test.tsx` — test that data-focused attribute is present on the correct field row
- [ ] Test that changing focusedIndex updates which field has the focus indicator
- [ ] Run: `cd kanban-app && npx vitest run src/components/entity-inspector.test.tsx`