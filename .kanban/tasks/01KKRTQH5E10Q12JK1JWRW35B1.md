---
assignees:
- claude-code
position_column: done
position_ordinal: fffffa80
title: 'FieldRenderer: self-contained field updates via useFieldUpdate'
---
## What

Make `FieldRenderer` call `useFieldUpdate` internally instead of taking an `onCommit` prop. It already has `entity` and `field` — everything needed to persist changes. This eliminates the boilerplate where every consumer (inspector, board cards, grid) manually wires `useFieldUpdate` → `handleCommit` → `onCommit`.

**Affected files:**
- `kanban-app/ui/src/components/field-renderer.tsx` — use `useFieldUpdate` internally, drop `onCommit` prop
- `kanban-app/ui/src/components/entity-inspector.tsx` — remove `handleCommit` boilerplate, use `FieldRenderer` without `onCommit`
- `kanban-app/ui/src/components/board-selector.tsx` — use `FieldRenderer` for the board name instead of `EditableMarkdown` with manual wiring
- Any other consumers of `FieldRenderer` that pass `onCommit`

**Approach:**
- `FieldRenderer` calls `useFieldUpdate().updateField(entity.entity_type, entity.id, field.name, value)` in its internal commit handler
- Remove the `onCommit` prop from `FieldRendererProps`
- Update all consumers to drop their `onCommit` wiring
- `useFieldUpdate` already returns a no-op when no provider exists, so this is safe in the quick-capture window

## Acceptance Criteria
- [ ] `FieldRenderer` persists field changes via `useFieldUpdate` internally
- [ ] `FieldRenderer` no longer has an `onCommit` prop
- [ ] EntityInspector no longer has `handleCommit` boilerplate
- [ ] BoardSelector uses `FieldRenderer` for the board name
- [ ] All existing field editing still works (inspector, grid, board cards)

## Tests
- [ ] `npm run typecheck` passes
- [ ] Existing tests pass
- [ ] Manual: edit a field in the inspector — persists correctly
- [ ] Manual: edit board name in the selector — persists correctly