---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: todo
position_ordinal: ff8a80
project: spatial-nav
title: 'Wrap app shell regions as zones: NavBar, Toolbar, tab bar, perspective, view container'
---
## What

Wrap the window chrome regions in `<FocusScope kind="zone">` so the app shell participates in the spatial-nav zone model. Without this, beam rule 1 (within-zone) has nothing to contain — focus would treat all chrome + content as a single flat layer of leaves, which gives worse locality.

### Regions to wrap

Inside the window root `<FocusLayer name="window">`:

- **`NavBar`** (`kanban-app/ui/src/components/nav-bar.tsx`) — `<FocusScope kind="zone" moniker="ui:navbar">` wrapping the whole nav bar. Children (logo, menu items, mode indicator) stay as default Leaves.
- **Toolbar / action groups** — wherever action buttons cluster (e.g., "new task", "search", "filter"). Wrap each logical group in `<FocusScope kind="zone" moniker="ui:toolbar.{group}">`.
- **Tab bar / perspective bar** (`PerspectivesContainer` or wherever the tab strip lives) — `<FocusScope kind="zone" moniker="ui:perspective-bar">`. Each tab is a Leaf.
- **Perspective container** (`PerspectiveContainer`) — `<FocusScope kind="zone" moniker="ui:perspective">` wrapping the active perspective's content area.
- **View container** (`ViewContainer`) — `<FocusScope kind="zone" moniker="ui:view">` wrapping the BoardView or GridView region.

Inside these, existing FocusScopes that wrap entities (tasks, columns, field rows) keep their existing monikers but may be upgraded to `kind="zone"` by the column/inspector migration cards.

### Monikers

Use the `"ui:region.qualifier"` convention from card `01KNQXW7HH...` for non-entity chrome. Monikers must be stable and unique per window — the zone's SpatialKey (ULID) provides per-mount uniqueness regardless.

### Why this matters

Without these zones, the inspector field row, the board card, and the navbar button all have `parent_zone = None` and are peers in beam rule 1. Arrow-up from a card in column 0 could land on the toolbar's filter button if the geometry happens to line up, instead of on the column header. With zones, beam rule 1 stays inside the board content, and only rule 2's fallback can reach chrome — which is what we want as an escape hatch.

### Files to modify

- `kanban-app/ui/src/components/nav-bar.tsx`
- `kanban-app/ui/src/components/perspectives-container.tsx` (tab bar)
- `kanban-app/ui/src/components/perspective-container.tsx`
- `kanban-app/ui/src/components/views-container.tsx` or `view-container.tsx`
- `kanban-app/ui/src/App.tsx` — ensure `<FocusLayer name="window">` wraps everything, and the above components are inside it

### Subtasks
- [ ] Wrap NavBar in `<FocusScope kind="zone">`
- [ ] Wrap toolbar / action groups (inventory which groups exist first)
- [ ] Wrap tab bar / perspective bar
- [ ] Wrap perspective container
- [ ] Wrap view container
- [ ] Verify the root `<FocusLayer name="window">` is in place at App root

## Acceptance Criteria
- [ ] NavBar, toolbar groups, tab bar, perspective, and view container each register as a Zone
- [ ] Zone hierarchy at a window root is: `window_root → [navbar_zone, toolbar_zone(s), perspective_zone → view_zone → board/grid content]`
- [ ] Beam rule 1 within board content stays inside the view zone → doesn't leak into navbar/toolbar
- [ ] Drill-out from a board card eventually reaches window-root level (zone nav traverses up through view → perspective → root)
- [ ] Existing React tests for these components still pass
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `nav-bar.test.tsx` — navbar wrapper registers with `kind="zone"`; children register with `parent_zone = navbar_zone_key`
- [ ] `perspectives-container.test.tsx` — tab bar zone; tab buttons as leaves
- [ ] Integration: arrow keys within the board view do not reach the nav bar via rule 1 (they only reach it via rule 2 fallback from edges, which is the intended escape hatch)
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.