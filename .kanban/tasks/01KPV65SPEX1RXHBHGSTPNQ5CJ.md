---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8f80
project: spatial-nav
title: 'Perspective tab: clicking the tab div should focus the scope'
---
## What

Investigate why a raw mouse click on a perspective tab `<div>` (inside `<PerspectiveTabBar />`) does not land `data-focused="true"` on the tab's FocusScope in the golden-path fixture. The existing tests in `spatial-nav-golden-path.test.tsx` (`perspective_j_from_focused_tab_dispatches_nav_down`, `perspective_scripted_nav_up_from_card_lands_on_active_tab`, `perspective_scripted_nav_down_from_tab_lands_on_view_content`, and `enter_on_perspective_tab_switches_active_perspective`) all reach the tab by seeding focus via a nav keypress from a top-row card (`click(card) → keyboard("k") → scripted focus-changed`). They avoid clicking the tab directly because the click does not cleanly focus the tab scope in the fixture.

This is the opposite of the golden-path philosophy: if mouse users can't click a tab to focus it, the tab is not fully reachable by mouse, and "nav works" is partially false for perspective tabs. Either:

- **(a)** Fix the fixture or production wiring so a raw click on the tab `<div>` focuses the scope directly — verify clicking the tab sets `data-focused="true"` without needing a nav-key seed, and update the existing golden-path tests to click the tab instead of the seed-via-card pattern.
- **(b)** Confirm that production (not just the fixture) works for mouse clicks, meaning this is a fixture-only artifact. In that case, document it clearly in `spatial-perspective-fixture.tsx` so future tests don't rediscover the workaround.

### Where to look

- `src/components/perspective-tab-bar.tsx` — `ScopedPerspectiveTab` uses `onClickCapture={handleScopeClick}` on the inner `<div>`, which calls `setFocus(tabMoniker)`. This should set focus, but the golden-path tests report it does not land `data-focused` in the fixture.
- `src/test/spatial-perspective-fixture.tsx` — the `PerspectiveStack` wrapper and `PERSPECTIVE_FIXTURE_CSS` overrides may be interfering with click-focus behavior.
- `src/test/spatial-nav-golden-path.test.tsx` — search for "click on the tab" and "seed via card" comments; those are the workaround sites.

## Acceptance Criteria

- [ ] Clicking a perspective tab `<div>` (via `userEvent.click(tabEl)`) in the golden-path fixture results in `tabEl.getAttribute("data-focused") === "true"` within `POLL_TIMEOUT`.
- [ ] Updated `perspective_j_from_focused_tab_dispatches_nav_down` (and the three related perspective tests) to click the tab directly instead of seeding via `card → k` when (a) is chosen.
- [ ] Updated `enter_on_perspective_tab_switches_active_perspective` similarly.
- [ ] Or if (b) is chosen: fixture comment in `spatial-perspective-fixture.tsx` and in `spatial-nav-golden-path.test.tsx` explicitly records the production-works / fixture-only split.

## Tests

- [ ] `cd kanban-app/ui && npm test -- spatial-nav-golden-path` — green.
- [ ] Manual: click a perspective tab in the running app and confirm the tab receives `data-focused="true"`.

## Workflow

Investigate, then choose (a) or (b). Do not skip documenting the result either way.

## Notes

Deferred from `01KPTT9X3HK5T7J5AMC6KHQHGQ`'s review (nit at line 977 of `spatial-nav-golden-path.test.tsx`): the existing workaround (seed focus via card → `k`) is clever but lets the mouse-click behavior hide. This follow-up closes that gap.