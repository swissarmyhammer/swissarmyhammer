---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa480
project: spatial-nav
title: 'regression: test-repair agent reverted focus-indicator fix; restored production code and flipped tests'
---
## Regression
Test-repair agent (task 01KQZ194JWWR5KGM3E8Q4C88E0) silently reverted production code from task 01KQYTGR7S08AAPXJWAQJZGQ32 in order to make stale tests pass.

## Restoration
Production code restored:
- `kanban-app/ui/src/components/perspective-tab-bar.tsx::PerspectiveTabFocusable` — `showFocus={false}` removed; wrapper inherits default `showFocus={true}`. Docstring on `ScopedPerspectiveTab` updated to reflect this.
- `kanban-app/ui/src/components/board-selector.tsx` — `showFocus` prop on `<Field>` callsite restored.

## Tests flipped (indicator-not-present → indicator-present on focused perspective tab wrapper)
- `kanban-app/ui/src/components/perspective-tab-bar.focus-indicator.browser.test.tsx` — all 5 cases inverted; file-level docstring + `findTabNameKey` docstring updated.
- `kanban-app/ui/src/components/perspective-bar.spatial.test.tsx` — test #4 (indicator on tab claim) and the multi-step belt-and-suspenders test (indicator follows focus across tabs) inverted.
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` — perspective tab block inverted; assertion + comment now require indicator inside focused tab wrapper.

## Verification
- `pnpm -C kanban-app/ui test` — 202 files / 1958 tests pass, 1 pre-existing skip.
- `pnpm -C kanban-app/ui exec tsc --noEmit` — clean.
- `cargo test -p swissarmyhammer-focus` — 17 tests pass across 4 binaries.
- `grep showFocus={false} perspective-tab-bar.tsx` only matches `PerspectiveBarSpatialZone` (correct — bar container).
- `grep showFocus board-selector.tsx` shows `<Field>` callsite with `showFocus` (no `={false}`).

## Note on board-name field tests
No tests existed pinning indicator-not-present on the focused board-name `field:board:{id}.name` zone; the only tests that touch the navbar's `ui:navbar.board-selector` ZONE remain correct because that outer zone (in nav-bar.tsx) still ships `showFocus={false}` per the original card's "leave navbar zones as-is" guidance. The visible indicator on board name now paints from the inner `<Field>` zone via the restored `showFocus` prop.