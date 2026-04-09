---
assignees:
- claude-code
depends_on:
- 01KNEZZ0PDQZHTGRY56Z2JCTBY
position_column: done
position_ordinal: ffffffffffffffffffffa280
title: Filter GroupSelector to only show `groupable` fields
---
## What

Update the frontend to consume the new `groupable` attribute and only show groupable fields in the group-by dropdown.

### Files to modify

- `kanban-app/ui/src/types/kanban.ts` — add `groupable?: boolean` to the `FieldDef` interface (line ~128)
- `kanban-app/ui/src/components/group-selector.tsx` — change line 64 from `fields.filter((f) => f.section !== "hidden")` to `fields.filter((f) => f.groupable === true)`

### Design

The filter uses strict `=== true` so that `undefined` (field has no `groupable` key) and `false` both exclude the field. This matches the opt-in backend design.

## Acceptance Criteria

- [ ] GroupSelector only lists fields where `groupable === true` in the dropdown
- [ ] Fields like title, body, attachments, description do NOT appear
- [ ] Fields like position_column (status), tags, assignees DO appear
- [ ] Selecting a groupable field still dispatches `perspective.group` correctly
- [ ] "None" option still dispatches `perspective.clearGroup`

## Tests

- [ ] Update `kanban-app/ui/src/components/group-selector.test.tsx`: pass a mix of groupable and non-groupable fields, assert only groupable ones render as options
- [ ] Add test: pass fields where none are groupable, assert only "None" option renders
- [ ] Run: `cd kanban-app/ui && npx vitest run group-selector` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.