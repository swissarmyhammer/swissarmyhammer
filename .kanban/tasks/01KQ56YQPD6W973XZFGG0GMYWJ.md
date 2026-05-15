---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb580
project: spatial-nav
title: 'Inspector: field icon + content stack vertically (FocusScope wrapper layout regression)'
---
## What

Regression in the inspector: each field row's icon and content now stack vertically (icon on top, value below) instead of laying out horizontally (icon | label | value).

Cards do NOT show this regression — likely because compact card cells use a different layout path than the full-size inspector field rows.

## Repro

1. Open the inspector on any task.
2. Look at the field rows (status, assignee, tags, etc.).
3. Observe: icon is above the field content, not beside it.

Compare to the same fields rendered on a card (compact display) — those still lay out horizontally.

## Likely cause

The Inspector field-rows task (01KNQY0P9J03T24FSM8AVPFPZ9) wrapped each field row in `<FocusScope kind=\"zone\">`. That wraps the row in a new DOM `<div>` (via the `<FocusZone>` primitive) which:

- May not forward the original row's flex layout classes (`flex flex-row items-center gap-…`).
- Or may have its own `display: block` / `display: flex flex-col` default that overrides the parent flex direction.

This is the same class of regression that hit `perspective-container` and `view-container` earlier — `<FocusZone>` injects DOM that needs explicit `className` to preserve the surrounding flex chain.

Why cards don't regress: the card path uses different display components (likely `BadgeListDisplay` rendering pills inline via `MentionView`); the inspector's `FieldRow` is a distinct component whose layout is now broken by the zone wrap.

## Root cause (verified during implementation)

The className IS forwarded to the FocusZone div (the outer FocusScope element), but `<FocusScope>` mounts a `<FocusScopeBody>` div inside the FocusZone, between the outer container and the children. That intermediate body div has no className and renders as `display: block`, so the icon span + content div collapse into block flow inside it and stack vertically. References like `BoardSpatialZone`, `ViewSpatialZone`, `nav-bar.tsx` use `<FocusZone>` directly (no inner body wrapper), so adding flex classes to their outer div is enough.

## Files

- `kanban-app/ui/src/components/entity-inspector.tsx` — `FieldRow` and the new `<FocusScope kind=\"zone\">` wrap
- `kanban-app/ui/src/components/focus-zone.tsx` (reference — its default DOM shape)
- `kanban-app/ui/src/components/focus-scope.tsx` (the composite that wraps FocusZone)

## Acceptance Criteria

- [x] Inspector field rows render horizontally: icon | label | value (matching pre-regression layout)
- [x] No double-row visual artifact in any field type (status, assignee, tags, dates, mentions, etc.)
- [x] Card cells continue to render correctly (no regression elsewhere)
- [x] Spatial-nav zone registration unchanged: each FieldRow still registers as a `<FocusScope kind=\"zone\">` with the field moniker
- [x] `pnpm vitest run` passes
- [ ] Manual: open inspector, visually verify horizontal layout

## Fix

In `entity-inspector.tsx` `FieldRow`, kept the flex classes on the outer `<FocusScope kind=\"zone\">` (so the field-row test surface and any future ancestor flex chain still see them) AND added an inner `<div className=\"flex items-start gap-2 w-full\">` wrapper around the icon span + content div. The inner wrapper is what actually lays out the icon and content as siblings, because the FocusScopeBody div between the FocusZone and the children otherwise collapses them into block flow.

## Tests

- [x] `entity-inspector.test.tsx` — added regression test \"field row outer element has flex row layout classes (icon + content stay horizontal)\" that asserts (a) the field-row outer element carries `flex`, `items-start`, `gap-2` (and is NOT `flex-col`), and (b) the icon span and the content div share a common flex-row ancestor inside the row.

## Workflow

- Use `/tdd` — write a failing test for the field row's layout class first, then fix.

## Origin

User-reported regression on 2026-04-26 during `/finish $spatial-nav` recovery work. Same class as the earlier `perspective-container` / `view-container` flex-chain breakage — the Inspector field-rows card slipped through without forwarding layout classes through the `<FocusScope kind=\"zone\">` wrap.

## Review Findings (2026-04-26 11:38)

### Warnings
- [x] `kanban-app/ui/src/components/entity-inspector.test.tsx:1179` — The new regression test \"field row outer element has flex row layout classes (icon + content stay horizontal)\" passes both with AND without the inner flex wrapper fix in the test environment. Verified empirically by removing the inner `<div className=\"flex items-start gap-2 w-full\">` wrapper and re-running the test — it still passed (1/1 in `entity-inspector.test.tsx`). The test does not catch the regression it claims to guard against. Root cause: `renderInspector()` (line 242) does not mount `<FocusLayer>` / `<SpatialFocusProvider>`, so `<FocusScope>` falls through to the no-spatial-context path (`focus-scope.tsx:391`) which renders a single `<div>` and never mounts `FocusScopeBody`. The bug only manifests when `FocusScopeBody` is present (production path). Suggested fix: wrap `renderInspector` in the spatial provider stack (mirror `inspectors-container.test.tsx` or `column-view.spatial-nav.test.tsx`), or carve out the new test into a separate `entity-inspector.spatial-nav.test.tsx` that does. Without that, the only true verification is the manual acceptance criterion (still unchecked) — visually opening the inspector. Until the test fails on the bug it claims to prevent, the regression can recur silently.

  **Resolution (2026-04-26):** Added a dedicated `renderInspectorWithSpatial` helper in `entity-inspector.test.tsx` that wraps `<EntityInspector>` in `<SpatialFocusProvider>` + `<FocusLayer name=\"window\">`, and switched the regression test to use it. Verified empirically:

  - Test PASSES with the inner `<div className=\"flex items-start gap-2 w-full\">` wrapper in place (current production code).
  - Test FAILS with the inner wrapper removed (replaced with `<>...</>`), with the diagnostic message \"content div is not a sibling of the icon span in the flex row\" — exactly the regression it's meant to catch.

  The full `entity-inspector.test.tsx` file (28 tests) and the full `pnpm vitest run` (143 files / 1567 tests) both pass. Other tests in this file continue to use the lighter-weight `renderInspector` helper since they don't depend on the production DOM shape.

### Nits
- [x] `kanban-app/ui/src/components/entity-inspector.tsx:312-321` — The duplicate-flex-classes pattern (outer FocusScope + inner wrapper) is now used in three places: `entity-card.tsx` (inner explicit div), `column-view.tsx` (absolute-positioned escape hatch), and now `entity-inspector.tsx`. The shared root cause is `FocusScopeBody` (`focus-scope.tsx:463-486`) inserting a className-less `<div>` that breaks any flex chain through `<FocusScope>`. Consider a follow-up task to fix this at the source — either give `FocusScopeBody` `display: contents` so it's transparent to layout, or forward `className` / `style` to it — so that future `<FocusScope kind=\"zone\">` consumers don't rediscover the same regression. The current per-call-site fixes are correct in isolation but the root architectural problem keeps biting.

  **Resolution (2026-04-26):** Spun out as separate task `01KQ5ACT9582HHD80GRBH1QMAP`. Per direction, the architectural fix is intentionally NOT attempted in this card — this card stays scoped to the inspector regression + its regression-guard test.