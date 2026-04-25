---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
position_column: todo
position_ordinal: ff9580
project: spatial-nav
title: 'NavBar: wrap as zone, strip legacy keyboard nav'
---
## What

Wrap the nav bar in `<FocusZone moniker="ui:navbar">` and strip every legacy keyboard-nav vestige from `nav-bar.tsx`. Children (logo, menu items, breadcrumbs, mode indicator) become leaves within the navbar zone.

### Files to modify

- `kanban-app/ui/src/components/nav-bar.tsx`

### Zone shape

```
window root layer
  ui:navbar (FocusZone) ← THIS CARD
    ui:navbar.logo (Leaf)
    ui:navbar.{menu_or_action} (Leaf, one per actionable item)
    ui:navbar.mode-indicator (Leaf)
```

### Legacy nav to remove

- Any `onKeyDown` listeners on the nav-bar div or its children (e.g. left/right arrows traversing menu items)
- Any document-level `keydown` listeners scoped to the nav bar
- Any imperative focus wiring (`useRef` + `.focus()` driven by keyboard handlers)
- `claimWhen` props or `ClaimPredicate` imports if present

What stays: button-click handlers (mouse), `aria-` attributes, focus-trap removal logic if any.

### Subtasks
- [ ] Wrap nav-bar content in `<FocusZone moniker={Moniker("ui:navbar")}>`
- [ ] Each actionable child becomes a `<Focusable moniker={Moniker("ui:navbar.{name}")}>` leaf (or a `<FocusScope>` if it represents an entity)
- [ ] Remove all keyboard listeners from nav-bar.tsx
- [ ] Remove `claimWhen` props / `ClaimPredicate` imports if present
- [ ] Audit imports: drop anything related to legacy nav (`useNavigation` hook, etc., if specific to the old system)

## Acceptance Criteria
- [ ] Nav bar registers as a `FocusZone` with `parent_zone = window root layer`
- [ ] All actionable children register as leaves with `parent_zone = ui:navbar`
- [ ] No `onKeyDown` / `keydown` / `useEffect`-bound listener in nav-bar.tsx
- [ ] Beam search rule 1 (within-zone) keeps arrow nav inside the nav bar when focus is on a navbar item
- [ ] `pnpm vitest run` passes

## Tests
- [ ] `nav-bar.test.tsx` — nav bar registers as a Zone; children register with `parent_zone = navbar zone key`
- [ ] `nav-bar.test.tsx` — no `keydown` event listener attached
- [ ] Integration: arrow nav within nav bar moves between leaves; cannot escape navbar via arrow alone (only via beam-rule-2 fallback)
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.