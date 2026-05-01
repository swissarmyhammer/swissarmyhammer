---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
- 01KQ5QB6F4MTD35GBTARJH4JEW
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc280
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

- [x] Manual smoke: clicking a field label shows a visible focus indicator
- [x] Manual smoke: clicking a pill in a badge list shows a visible focus indicator
- [x] Manual smoke: clicking an inline editor (when not in edit mode) shows visible focus
- [x] Manual smoke: drilling out (Escape) from a pill lands focus on its enclosing field row with visible feedback
- [x] Integration test per leaf + zone-level focus
- [x] Existing inspector / badge-list tests stay green

## Tests

- [x] `entity-inspector.spatial-nav.test.tsx` — click each kind of leaf → assert visible indicator
- [x] `badge-list-nav.test.tsx` — click each pill → assert visible indicator
- [x] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow

- Use `/tdd` — write the integration test first, watch it fail, then fix.

---

## Implementation Notes (2026-04-26)

### MentionView fix

`mention-view.tsx` `MentionViewList` no longer hard-suppresses `showFocusBar` based on `mode`. Pre-fix, every pill in `mode="compact"` was forced to `showFocusBar={false}` — that is what blocked the entity-card card's "clicking an assignee pill produces a visible indicator" criterion. Post-fix, both compact and full modes pass `props.showFocusBar` through unchanged, so each pill defaults to `<FocusScope>`'s default of `true` and any caller that needs suppression passes `showFocusBar={false}` explicitly. `MentionViewProps.mode` is retained on the public surface for callers that thread mode through (badge-list-display) and as a hook for future mode-specific tweaks; its docstring now flags it as informational at this layer.

### Field-as-zone docstrings (audit item #3)

`fields/field.tsx`'s top-level docstring and the `showFocusBar` prop docstring previously said the inspector row "provides its own visual context" so the zone bar should default to off. That contradicted the actual `entity-inspector.tsx` callsite, which passes `<Field showFocusBar />`. Updated both docstrings to reflect the deliberate callsite truth: grid cells provide their own cursor ring, but inspector rows fill the panel width with no enclosing chrome, so the per-row bar IS the user's only focus cue at that level — opt-in is correct. No code change to the runtime behaviour; only the comments.

### Tests added

- `entity-inspector.spatial-nav.test.tsx` (NEW, 6 tests) — production-shaped spatial-nav stack mounting `EntityInspector`. Covers: click → `spatial_focus` for single-value field rows; click → `spatial_focus` for badge-list pill leaves (verifying leaf wins over zone via `e.stopPropagation`); focus-claim → `<FocusIndicator>` mount on single-value zones, computed display-only (`progress`) zones, and badge-list pill leaves; drill-out from pill to field-row zone moves the visible indicator with the focus.
- `badge-list-nav.test.tsx` (EXTENDED, +4 tests) — kept the existing 3 structural tests; rewired the file to use hoisted Tauri mocks compatible with the spatial provider stack; added a new `BadgeListDisplay pill click → visible focus indicator` describe block: click → `spatial_focus`; focus-claim → indicator in `mode="full"`; focus-claim → indicator in `mode="compact"` (the regression the user reported); only-one-bar-at-a-time as focus moves between sibling pills.

### Test results

- `cd kanban-app/ui && npx vitest run`: 153 files passed, 1669 tests passed, 1 skipped, 0 failed.
- `cd kanban-app/ui && npx tsc --noEmit`: clean.

## (Prior) Implementation Notes (2026-04-26)

Field rows became `<FocusScope kind="zone">`; `claimPredicates` memo + `useFieldClaimPredicates` hook + `predicatesForField` + `edgePredicates` deleted (~90 lines). `fieldMonikers` memo replaced with single `firstFieldMoniker` for mount-time first focus. `claimWhen` prop and `ClaimPredicate` import gone from `<FieldRow>` and inner `<FocusScope>`. `mention-view.tsx` lost `buildListClaimPredicates` and dropped `claimWhen` from `MentionViewProps` / `SingleMentionProps`. ~155 net lines removed across both files. All 1538 tests passed at completion.