---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: todo
position_ordinal: ff8a80
project: spatial-nav
title: 'Perspective: wrap perspective bar + container + view as zones, strip legacy nav'
---
## What

Wrap the perspective system as zones — the perspective tab bar, the perspective container, and the inner view container. Each is its own zone, nested. Strip every legacy keyboard-nav vestige from these files.

NavBar and toolbar are split into their own cards (separate components, separate zones).

### Zone shape

```
window root layer
  ui:perspective-bar (FocusZone)        ← perspectives-container.tsx tabs
    perspective_tab:{id} (Leaf, per tab)
  ui:perspective (FocusZone)            ← active perspective
    ui:view (FocusZone)                 ← BoardView or GridView selector
      ui:board OR ui:grid (FocusZone, separate cards)
```

### Files to modify

- `kanban-app/ui/src/components/perspectives-container.tsx` — the tab bar (or a child component if the tab strip lives elsewhere; check for `perspective-tab-bar.tsx`)
- `kanban-app/ui/src/components/perspective-container.tsx` — the active perspective wrapper
- `kanban-app/ui/src/components/views-container.tsx` and/or `view-container.tsx` — the view selector / wrapper

### Legacy nav to remove

- Any `onKeyDown` listeners on the tab strip (left/right arrow handling, Enter to switch) — these become spatial nav between sibling tab leaves at the perspective-bar zone level
- Any `useEffect` keyboard listeners
- Any `claimWhen` props or `ClaimPredicate` imports
- Any imperative `tabRef.current?.focus()` calls wired to keyboard handlers

What stays: tab-click dispatch (mouse), `aria-` attributes for accessibility.

### Subtasks
- [ ] Wrap perspective tab bar in `<FocusZone moniker={Moniker("ui:perspective-bar")}>`
- [ ] Each tab inside is a `<Focusable moniker={Moniker(`perspective_tab:${id}`)}>` leaf
- [ ] Wrap the active perspective in `<FocusZone moniker={Moniker("ui:perspective")}>`
- [ ] Wrap view container in `<FocusZone moniker={Moniker("ui:view")}>`
- [ ] Remove `onKeyDown` / `keydown` listeners from perspective-tab-bar.tsx, perspectives-container.tsx, perspective-container.tsx, views-container.tsx, view-container.tsx
- [ ] Remove `claimWhen` props / `ClaimPredicate` imports if present

## Acceptance Criteria
- [ ] Three zones registered at startup: `ui:perspective-bar`, `ui:perspective`, `ui:view` (when on a perspective with a view)
- [ ] Each tab is a leaf within `ui:perspective-bar`; arrow keys move between tabs via spatial nav, Enter activates via global `nav.drillIn` (fires the existing `view.set` / `perspective.set` command)
- [ ] No keyboard listeners in any of the perspective files
- [ ] Existing perspective tests pass
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `perspective-tab-bar.test.tsx` — tab bar registers as a Zone; tabs are leaves
- [ ] `perspective-container.test.tsx` — perspective container is a Zone with `parent_zone` = window root
- [ ] `views-container.test.tsx` — view container is a Zone inside the perspective zone
- [ ] Integration: arrow keys traverse tabs (via beam search, sibling leaves in the same parent_zone)
- [ ] No `keydown` listener attached in any of the modified files
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.