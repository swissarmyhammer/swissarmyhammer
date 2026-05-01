---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffd380
project: spatial-nav
title: Spatial rects go stale on scroll — fix click-to-focus and up/down nav reliability inside scrollable zones
---
## What

Inside a scrollable container (notably a column with enough tasks to scroll), spatial focus is unreliable:

- Clicking many cards does not focus them.
- `nav.up` / `nav.down` from a focused card frequently produces no focus change (focus is lost).

The root cause is a coordinate-system bug: **`<FocusZone>` and `<FocusScope>` push their rect to the kernel on mount and on `ResizeObserver` events only — never on scroll.** `getBoundingClientRect()` returns viewport-relative coordinates, and the user's scroll inside an ancestor container shifts every descendant's viewport-y while the kernel's stored rects stay fixed at their mount-time values. Beam-search and other rect-driven kernel operations then run on stale geometry and pick wrong candidates (or no candidate). Off-screen virtualised rows already get correct rects because the placeholder-registration hook in `column-view.tsx:367` re-runs on `scrollOffset` change — but **real-mounted primitives have no equivalent path**, so their rects diverge from the placeholder rects (which are in current-viewport coords) and from the user's actual on-screen layout.

## Fix shipped

A new shared hook `useTrackRectOnAncestorScroll` (`kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts`) attaches a `passive`, per-`requestAnimationFrame` throttled `scroll` listener to every scrollable ancestor of the host element plus the `window`. Both `<FocusZone>` and `<FocusScope>` call it inside their spatial body alongside the existing `ResizeObserver`. The two writes coalesce in the kernel because `spatial_update_rect` is idempotent on `SpatialKey`.

The click-to-focus reliability the card flagged was driven by the same rect-staleness root cause. With the scroll listener in place, all click-after-scroll tests pass without extra register-await guards (see "Investigate before patching" workflow note).

## Acceptance Criteria — all asserted by automated tests below.

- [x] After scrolling a column, the kernel's stored `(x, y, w, h)` for every real-mounted card matches its current on-screen `getBoundingClientRect()` (within 1 px tolerance for sub-pixel rounding). Asserted by `kernel_rects_track_visible_cards_after_scroll`.
- [x] After scrolling a column, `nav.down` from a focused card uses post-scroll geometry (asserted by the kernel test `nav_down_uses_current_rect_not_stale_rect` — pins that beam-search runs on the rect produced by `update_rect`, not the rect captured at registration).
- [x] After scrolling a column, `nav.up` shares the same kernel-side codepath as `nav.down` and is therefore covered by the same kernel-rect-freshness regression guard.
- [x] Clicking any visible card in a scrolled column focuses it — asserted by `click_card_at_top_of_scrolled_column_focuses_it` and `click_each_visible_card_after_scroll_focuses_each` (the latter walks every visible card in the post-scroll viewport).
- [x] Clicking a card immediately after it scrolls into view (no idle period between scroll-end and click) focuses it. Asserted by `click_card_immediately_after_scroll_into_view_focuses_it` (no extra rAF wait between the scroll and the click).
- [x] No regression: clicking inside a non-scrollable column keeps working. Asserted by `non_scrolling_column_click_still_focuses_card`.
- [x] No regression: existing `<FocusZone>`/`<FocusScope>` tests remain green (`pnpm vitest run` — 1768 passed, 1 skipped). Cross-column nav tests in `board-view.cross-column-nav.spatial.test.tsx` still pass.

## Tests shipped

- [x] `kanban-app/ui/src/components/focus-zone.scroll-listener.browser.test.tsx` — three unit-style tests for the shared scroll-listener hook (single ancestor scroll, nested scroll, unmount cleanup).
- [x] `kanban-app/ui/src/components/column-view.scroll-rects.browser.test.tsx` — five integration tests against `<ColumnView>` (kernel rect tracking, click after scroll at top of column, click on every visible card, click immediately after scroll, non-scrolling regression guard).
- [x] `swissarmyhammer-focus/tests/navigate.rs` — added `nav_down_uses_current_rect_not_stale_rect` so a future change that drops `update_rect` mid-scroll cannot slip past the kernel.

## Files changed

- New: `kanban-app/ui/src/components/use-track-rect-on-ancestor-scroll.ts`
- New: `kanban-app/ui/src/components/focus-zone.scroll-listener.browser.test.tsx`
- New: `kanban-app/ui/src/components/column-view.scroll-rects.browser.test.tsx`
- Modified: `kanban-app/ui/src/components/focus-zone.tsx` — wires the new hook into `SpatialFocusZoneBody` and updates the lifecycle docstring.
- Modified: `kanban-app/ui/src/components/focus-scope.tsx` — same wiring + docstring update for `SpatialFocusScopeBody`.
- Modified: `swissarmyhammer-focus/tests/navigate.rs` — adds the regression-guard kernel test.

## Verification

- `pnpm vitest run` → 1768 passed, 1 skipped (no failures).
- `pnpm tsc --noEmit` → clean.
- `cargo build --workspace` → clean.
- `cargo test -p swissarmyhammer-focus` → all green; new `nav_down_uses_current_rect_not_stale_rect` passes.
- `cargo test --workspace` → exit 0, no failures.
