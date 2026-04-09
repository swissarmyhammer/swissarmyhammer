---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Add tooltips to field icons in EntityCard and extract shared icon utilities
---
## What

Field icons in the **EntityCard** (`kanban-app/ui/src/components/entity-card.tsx:113-123`) render as plain `<span>` elements with no tooltip. The **EntityInspector** (`kanban-app/ui/src/components/entity-inspector.tsx:336-347`) wraps the same icons in `<Tooltip>/<TooltipTrigger>/<TooltipContent>` so users get a hover label showing `field.description || field.name`. The card view should behave the same way.

Both files also duplicate `kebabToPascal` and icon resolution logic:
- `entity-card.tsx:17-30` — `kebabToPascal` + `resolveIcon`
- `entity-inspector.tsx:23-36` — `kebabToPascal` + `fieldIcon`

### Approach

1. Extract a shared `fieldIcon(field: FieldDef)` utility into a new file `kanban-app/ui/src/components/fields/field-icon.ts` that both components import. This replaces `resolveIcon` (card) and `fieldIcon` (inspector), and eliminates the duplicated `kebabToPascal`.

2. In `entity-card.tsx`, wrap the icon `<span>` in `<Tooltip>/<TooltipTrigger>/<TooltipContent>` using `field.description || field.name.replace(/_/g, " ")` as the tooltip text — matching the inspector's behavior.

### Subtasks

- [ ] Create `kanban-app/ui/src/components/fields/field-icon.ts` exporting `fieldIcon(field: FieldDef): LucideIcon | null` (returns `null` when no icon, unlike inspector's `HelpCircle` fallback — card should not show fallback icons)
- [ ] Update `entity-inspector.tsx` to import `fieldIcon` from the shared module, remove local `kebabToPascal` and `fieldIcon`
- [ ] Update `entity-card.tsx` to import `fieldIcon` from the shared module, remove local `kebabToPascal` and `resolveIcon`, wrap icon in Tooltip
- [ ] Add unit test for shared `fieldIcon` utility

## Acceptance Criteria

- [ ] Hovering over a field icon in EntityCard shows a tooltip with the field description (or humanized field name)
- [ ] `kebabToPascal` exists in exactly one location (the shared module)
- [ ] EntityInspector tooltip behavior is unchanged
- [ ] No regressions in existing entity-card or entity-inspector tests

## Tests

- [ ] `kanban-app/ui/src/components/fields/field-icon.test.tsx` — unit tests for `fieldIcon`: returns correct icon for known names, returns `null` for missing icon, handles kebab-case conversion
- [ ] Update `kanban-app/ui/src/components/entity-card.test.tsx` — add test that field icons render with `role="button"` or tooltip trigger, and tooltip content matches field description
- [ ] Run `cd kanban-app/ui && npx vitest run --reporter=verbose` — all existing tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.