---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc580
project: spatial-nav
title: 'MentionView: show focus indicator on compact-mode pills (assignee/tag pills in cards)'
---
## What

`MentionView` currently passes `showFocusBar={false}` to every pill `<FocusScope>` when rendering in `mode="compact"`. See `kanban-app/ui/src/components/mention-view.tsx` `MentionViewList`:

```tsx
showFocusBar={mode === "full" ? props.showFocusBar : false}
```

This means tag pills and assignee pills inside cards (where `<Field mode="compact">` is used) NEVER show a visible focus indicator when clicked, even though:

- The pill registers as a `<FocusScope>` leaf in the spatial graph.
- The pill click correctly dispatches `spatial_focus` for the pill's key.
- The Rust kernel emits `focus-changed` and the React-side claim subscription fires.
- `data-focused="true"` flips on the pill's outer div.

The user-visible regression: "clicks fire spatial_focus, but no visible focus indicator appears on the leaf the user clicks" persists for pill leaves in cards. Title and status (single-value text fields) are covered by card `01KQ20NMRQ` because that card passed `showFocusBar={true}` to `<Field>` from `<CardField>`, and Field forwards to its outer zone — which IS the click target for single-value fields. Pills are different: they're inner leaves under MentionView, and MentionView's compact-mode override suppresses their focus bar.

## Decision needed

Two paths, both with the same end effect:

### Option A — flip the MentionView default for compact mode

Change `mention-view.tsx` to no longer hard-suppress `showFocusBar` in compact mode. Instead, propagate `props.showFocusBar` through both modes (or accept a separate `pillsShowFocusBar` prop). Compact-mode consumers that genuinely don't want a per-pill indicator (if any exist) opt out explicitly.

Pros: Simplest fix. Touches one file.
Cons: May affect other compact-mode consumers (grid cells in particular). Audit every `<MentionView mode="compact">` call site before flipping.

### Option B — propagate `pillsShowFocusBar` through `<Field>`

Add a `pillsShowFocusBar?: boolean` prop on `<Field>` that flows into `BadgeListDisplay` and on into `MentionView`. Card consumers (`<CardField>`) pass `pillsShowFocusBar={true}`; grid cells leave it default.

Pros: Most surgical. Each consumer of `<Field>` decides for itself.
Cons: Adds a prop that has to be wired through three components.

Recommend Option A as a starting point, with an audit of every compact-mode call site first to confirm no surprise indicator pop-ups appear in grid cells.

## Files involved

- `kanban-app/ui/src/components/mention-view.tsx` (the suppression site)
- `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` (forwards mode/showFocusBar to MentionView)
- `kanban-app/ui/src/components/fields/field.tsx` (forwards showFocusBar to its outer zone, may need pillsShowFocusBar prop too if Option B)
- Audit: every `mode="compact"` call site of `<Field>` and every `<MentionView mode="compact">` call site

## Acceptance Criteria

- [ ] Manual smoke: clicking an assignee pill in a card produces a visible focus indicator on the pill
- [ ] Manual smoke: clicking a tag pill in a card produces a visible focus indicator on the pill
- [ ] Existing compact-mode consumers (grid cells, etc.) audited; behaviour intentional and documented
- [ ] Browser test in `kanban-app/ui/src/components/entity-card.spatial.test.tsx` extended with two cases asserting `<FocusIndicator>` mounts on the focused tag pill / assignee pill after `fireFocusChanged({ next_key: pillKey })`
- [ ] All existing card / grid / inspector tests stay green

## Tests

- [ ] Extend `entity-card.spatial.test.tsx` per-leaf block: `focus claim on a tag pill mounts a visible FocusIndicator on the pill`
- [ ] Same for an assignee pill
- [ ] If Option A: extend `mention-view.test.tsx` with a compact-mode test asserting `showFocusBar` is now propagated (or document the new contract)
- [ ] `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- TDD: extend the entity-card.spatial test first with the two failing cases, then implement, then verify the extended test goes green.

## Origin

Spawned from `01KQ20NMRQQSXVRHP4RHE56B0K` (Card: wrap as zone). That card established the title and status indicator visibility fix in-turf (`<CardField>` passes `showFocusBar={true}` to `<Field>`), but the pill case requires changes in `mention-view.tsx` which is out of turf for the card's per-component scope.