---
assignees:
- claude-code
depends_on:
- 01KN4NGQY7NMEZ0HGDN10RVMWH
- 01KN4NG67RE9YSA4J3Q25YM98R
position_column: done
position_ordinal: ffffffffffffffffffffffc380
title: 9. Perspective sort commands + scope chain wiring
---
## What

Add field-level sort commands that flow through the scope chain `window > view > perspective > entity > field`, appearing in right-click menus, command palette, and grid column headers.

**Backend files to create/modify:**
- `swissarmyhammer-commands/builtin/commands/perspective.yaml` — add `perspective.sort.set`, `perspective.sort.clear`, `perspective.sort.toggle` commands
- `swissarmyhammer-kanban/src/dispatch.rs` — handle new sort commands (extract field from args, update perspective sort entries)
- `swissarmyhammer-kanban/src/perspective/` — may need a dedicated sort operation or extend UpdatePerspective

**Frontend files to modify:**
- `kanban-app/ui/src/components/grid-view.tsx` — column headers get CommandScopeProvider with field moniker + sort commands, click handler dispatches toggle, sort indicator rendered
- `kanban-app/ui/src/components/entity-inspector.tsx` — field rows could get sort commands in scope (context menu)
- `kanban-app/ui/src/lib/perspective-context.tsx` — provide perspective scope in provider so commands resolve perspective_id

**Approach:**

### Backend commands
```yaml
- id: perspective.sort.set
  name: Sort Field
  params:
    - name: field
      from: args
    - name: direction
      from: args  # "asc" | "desc"
    - name: perspective_id
      from: args
  keys: {}

- id: perspective.sort.clear
  name: Clear Sort
  params:
    - name: field
      from: args
    - name: perspective_id
      from: args
  keys: {}

- id: perspective.sort.toggle
  name: Toggle Sort
  params:
    - name: field
      from: args
    - name: perspective_id
      from: args
  keys: {}
```

### Scope chain
- PerspectiveProvider wraps children in `CommandScopeProvider` with moniker `perspective:{id}` and perspective-level commands
- Grid column headers wrap in `CommandScopeProvider` with moniker `field:{field_id}` providing sort commands
- When a sort command fires, the scope chain resolves: field provides `field` ULID, perspective scope provides `perspective_id`
- Commands dispatch to backend which updates the perspective's sort entries

### Grid column header behavior
- Click toggles sort: none → asc → desc → none
- Shift+click adds to multi-sort (appends to sort list)
- Sort indicator: chevron up/down + priority number (1, 2, 3...) for multi-sort
- Right-click shows "Sort Ascending", "Sort Descending", "Clear Sort"

### Command palette
- Sort commands appear in palette when a field or grid column is focused
- Palette shows field name in command label: "Sort: Title Ascending"

## Acceptance Criteria
- [ ] `perspective.sort.set` command updates perspective sort entries in backend
- [ ] `perspective.sort.clear` removes a field from sort entries
- [ ] `perspective.sort.toggle` cycles none → asc → desc → none
- [ ] Grid column header click triggers sort toggle
- [ ] Sort indicator (chevron + priority) renders in grid column headers
- [ ] Right-click on grid column header shows sort commands
- [ ] Sort commands appear in command palette when field is in scope
- [ ] Multi-sort works with shift+click

## Tests
- [ ] Backend: `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — sort command dispatch tests
- [ ] Frontend: `kanban-app/ui/src/components/grid-view.test.tsx` — column header sort indicator, click toggles sort
- [ ] `pnpm test` from `kanban-app/ui/` passes