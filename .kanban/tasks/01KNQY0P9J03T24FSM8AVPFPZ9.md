---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: doing
position_ordinal: '8580'
project: spatial-nav
title: 'Inspector and badge-list: wrap field rows as zones, delete claimWhen predicates'
---
## STATUS: REOPENED 2026-04-26 — does not work in practice

The user reports that **fields in inspectors cannot be focused or selected**. The structural wrapping shipped (field rows as zones, labels and pills as leaves), but clicking a field, label, or pill does not produce visible focus feedback. See umbrella card `01KQ5PEHWT...` for the systemic root-cause checklist.

## Remaining work

1. **Verify the click → indicator-rendered chain** for each leaf inside an inspector field row:
   - field label leaf
   - inline editor leaf (when present)
   - badge-list pill leaves (per pill)
2. Each field row is a zone (`<FocusScope kind="zone">`). If a row's body fills the whole visible width and has no clickable whitespace, ensure the row-level focus is reachable via drill-out. Verify the zone-level focus indicator is visible (or document why suppressed).
3. Walk the focus-changed event path with dev console open: click a label, click a pill, click an editor — for each, watch the Tauri event, the React claim, the indicator render.
4. Integration tests per leaf and per zone-level focus.

## Files involved

- `kanban-app/ui/src/components/entity-inspector.tsx`
- `kanban-app/ui/src/components/mention-view.tsx`
- `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx`
- `kanban-app/ui/src/components/focusable.tsx` / `focus-zone.tsx` / `focus-indicator.tsx`

## Acceptance Criteria

- [ ] Manual smoke: clicking a field label shows a visible focus indicator
- [ ] Manual smoke: clicking a pill in a badge list shows a visible focus indicator
- [ ] Manual smoke: clicking an inline editor (when not in edit mode) shows visible focus
- [ ] Manual smoke: drilling out (Escape) from a pill lands focus on its enclosing field row with visible feedback
- [ ] Integration test per leaf + zone-level focus
- [ ] Existing inspector / badge-list tests stay green

## Tests

- [ ] `entity-inspector.spatial-nav.test.tsx` — click each kind of leaf → assert visible indicator
- [ ] `badge-list-nav.test.tsx` — click each pill → assert visible indicator
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Implementation Notes (2026-04-26)

Field rows became `<FocusScope kind="zone">`; `claimPredicates` memo + `useFieldClaimPredicates` hook + `predicatesForField` + `edgePredicates` deleted (~90 lines). `fieldMonikers` memo replaced with single `firstFieldMoniker` for mount-time first focus. `claimWhen` prop and `ClaimPredicate` import gone from `<FieldRow>` and inner `<FocusScope>`. `mention-view.tsx` lost `buildListClaimPredicates` and dropped `claimWhen` from `MentionViewProps` / `SingleMentionProps`. ~155 net lines removed across both files. All 1538 tests passed at completion.