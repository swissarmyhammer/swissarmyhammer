---
assignees:
- claude-code
position_column: todo
position_ordinal: a980
project: spatial-nav
title: FieldIconBadge trigger should carry an aria-label for screen readers
---
## What

Pre-existing accessibility gap: `kanban-app/ui/src/components/fields/field-icon-badge.tsx` mounts the icon trigger as a bare `<span>` with no accessible name — only the Radix `<TooltipContent>` carries the description text. The tooltip is hover-/focus-triggered; until it opens, screen readers see an empty span with a decorative `<svg>` and no name.

## Background

This was discovered while reviewing `01KQAWV9C5F8Y3AA0KDDHHRRN1` (card-fields-render-through-Field-withIcon migration). The legacy `CardFieldIcon` component (now deleted) carried `aria-label={tip}` on its trigger span, so the icon always had a non-visual accessible name. After the migration both the card and inspector surfaces share `<FieldIconBadge>`, so the gap is now visible on both.

The reviewer flagged this as a nit and explicitly out of scope for the migration card.

## Where the fix lives

`kanban-app/ui/src/components/fields/field-icon-badge.tsx` line ~39 — the `<span>` inside `<TooltipTrigger asChild>`. Add `aria-label={tip}` (or `aria-describedby` pointing at the tooltip content id) so the icon has a non-visual accessible name regardless of tooltip-open state.

## Acceptance Criteria

- [ ] `<FieldIconBadge>`'s trigger `<span>` carries `aria-label={tip}` (or equivalent).
- [ ] Existing tests (`entity-card.field-icon-inside-zone.browser.test.tsx`, `field.with-icon.browser.test.tsx`, the inspector tooltip tests) continue to pass.
- [ ] Add a test asserting the trigger span has the expected accessible name for both the static-description path and the value-dependent `tooltipOverride` path.

## Tests

### Frontend — augment `kanban-app/ui/src/components/entity-card.field-icon-inside-zone.browser.test.tsx`

- [ ] Assert `[data-segment="field:task:T1.tags"] span[data-slot="tooltip-trigger"]` carries `aria-label="Task tags"`.
- [ ] In the `tooltipOverride` test, assert the trigger's `aria-label` equals the override string ("All good").

### Frontend — augment `kanban-app/ui/src/components/entity-inspector.test.tsx` (or equivalent)

- [ ] Assert the inspector's icon trigger carries the same `aria-label`.
