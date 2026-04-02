---
assignees:
- claude-code
depends_on:
- 01KN4NH98Q3Z479FBQP2SCYDX0
- 01KN4NJ21PSS15MAR0R542SYWF
position_column: todo
position_ordinal: '8580'
title: 10. Group-by selector + perspective scope command
---
## What

Add a group-by field selector that sets the perspective's group expression, following the same command/scope pattern as sort.

**Files to create:**
- `kanban-app/ui/src/components/group-selector.tsx` — dropdown field picker for group-by

**Files to modify:**
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — add group-by icon/button next to filter
- `kanban-app/ui/src/components/board-view.tsx` — use group expression to override swimlane grouping
- `kanban-app/ui/src/components/grid-view.tsx` — use group expression for row grouping (collapsible sections)

**Approach:**
- Group icon on the active perspective tab (next to filter icon)
- Click opens a dropdown listing available fields from the entity schema
- Selecting a field sets `group` to `(entity) => entity.{fieldName}` via `backendDispatch({ cmd: "perspective.group", args: { group, perspective_id } })`
- "None" option clears grouping via `perspective.clearGroup`
- Icon highlighted when group is active
- Board view: group expression determines swimlane assignment (entities grouped by expression result)
- Grid view: rows grouped into collapsible sections by group expression result

**Scope chain integration:**
- `perspective.group.set` and `perspective.group.clear` commands at perspective scope level
- These are simpler than sort — no field-level scope needed, just perspective scope

## Acceptance Criteria
- [ ] Group-by icon visible on active perspective tab
- [ ] Click opens field picker dropdown
- [ ] Selecting a field sets the group expression on the perspective
- [ ] "None" clears the group
- [ ] Board view groups entities by group expression
- [ ] Grid view shows grouped rows with section headers
- [ ] Icon highlighted when group is active

## Tests
- [ ] `kanban-app/ui/src/components/group-selector.test.tsx` — renders, selecting field sets group, clear removes group
- [ ] `pnpm test` from `kanban-app/ui/` passes