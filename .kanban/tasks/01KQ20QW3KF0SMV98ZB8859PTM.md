---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: todo
position_ordinal: ff9680
project: spatial-nav
title: 'Toolbar: wrap action groups as zones, strip legacy keyboard nav'
---
## What

Inventory the toolbar / action button groups in the app shell, wrap each logical group in `<FocusZone moniker="ui:toolbar.{group}">`, and strip every legacy keyboard-nav vestige.

The "toolbar" is wherever clusters of action buttons live (e.g. New Task, Search, Filter, Sort, Group). It may be inside the nav bar or as a sibling region — the inventory step finds out.

### Files to inventory & modify

Sweep `kanban-app/ui/src/components/` for files that render groups of action buttons. Likely candidates:
- `nav-bar.tsx` (parts of it may be toolbar groups, e.g. the right-side actions)
- `app-shell.tsx`
- Any component named `*-toolbar.tsx` or `*-actions.tsx`

Document in this card which files get zone wrappers and which don't (e.g. some action clusters may be too small to merit their own zone — but each cluster of 3+ peer leaves should be a zone).

### Zone shape

```
window root layer
  ui:toolbar.actions (FocusZone) — primary action cluster
    new_task (Leaf), filter (Leaf), sort (Leaf), search (Leaf), ...
  ui:toolbar.{other-group} (FocusZone) — additional cluster, if any
    ...leaves
```

### Legacy nav to remove

- `onKeyDown` listeners on toolbar buttons (Tab navigation between them)
- Any `keydown` `useEffect` listeners scoped to the toolbar
- Imperative `ref.focus()` wiring driven by keyboard handlers
- `claimWhen` props / `ClaimPredicate` imports
- Any roving-tabindex implementations (those are replaced by spatial nav within the zone)

What stays: `aria-` attributes, click handlers (mouse), keyboard shortcuts that are *commands* (e.g. Cmd+N for New Task — those live in the command registry, not as inline `onKeyDown`).

### Subtasks
- [ ] Inventory action button groups; document which files / which DOM regions become zones
- [ ] Wrap each group in `<FocusZone moniker={Moniker("ui:toolbar.{group}")}>`
- [ ] Each button becomes a `<Focusable moniker={Moniker("ui:toolbar.{group}.{action}")}>` leaf
- [ ] Remove `onKeyDown` / `keydown` listeners from each modified file
- [ ] Remove `claimWhen` / `ClaimPredicate` if present
- [ ] Remove roving-tabindex code if present (search for `tabIndex={-1}` patterns wired to keyboard handlers)

## Acceptance Criteria
- [ ] Each toolbar action cluster registers as a `FocusZone` with appropriate moniker
- [ ] Buttons within a cluster are leaves, with `parent_zone = the zone key`
- [ ] No `onKeyDown` / `keydown` listeners in toolbar files for navigation purposes
- [ ] Spatial nav within a toolbar zone (beam rule 1) traverses between buttons; rule 2 escapes to other zones
- [ ] `pnpm vitest run` passes

## Tests
- [ ] For each modified file, a test verifying the zone wrapping + leaves
- [ ] Integration: arrow nav within a toolbar cluster moves between buttons; can't escape via arrow alone within rule 1
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.