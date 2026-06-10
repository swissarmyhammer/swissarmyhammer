---
assignees:
- claude-code
position_column: todo
position_ordinal: e380
title: Pre-existing failures in entity-inspector.field-vertical-nav.browser.test.tsx (4 tests) — stale spatial_navigate expectations, untracked by existing breakage cards
---
## What

Discovered while implementing 01KTCQF7J3YZ1SAY0W96WWA35X (jump targets regression): `apps/kanban-app/ui/src/components/entity-inspector.field-vertical-nav.browser.test.tsx` fails 4/4 tests, and this file is NOT covered by either pre-existing-breakage card (01KTQ8KRJYX1DPHN76TZ654ZX2 or 01KTQEKP9E8TPQ547BWA5RGWH9).

Failure shape: `spatialNavigateCalls()` returns 0 where the tests expect >= 1 after an ArrowDown/ArrowUp keydown — the same class as the tracked nav-rework stale-harness failures (nav commands now execute host-side via `dispatch_command nav.up/down`; the webview no longer fires a `spatial_navigate` IPC the harness can count). Failing tests:

- down_from_first_field_lands_on_second_field
- up_from_last_field_lands_on_previous_field
- down_after_scroll_picks_next_field_in_content_order
- down_at_last_visible_field_scrolls_to_bring_next_field_into_view

Verified independent of the jump-targets fix: the file renders `EntityInspector` + `AppShell` and imports none of the modules changed by that card (jump-to-overlay.tsx, slide-panel.tsx, inspectors-container.tsx).

NOTE: the jump-targets card REPAIRED the same wire-shape staleness in jump-to-overlay.browser.test.tsx, jump-to-overlay.window-layer.browser.test.tsx, jump-to-overlay.over-inspector.browser.test.tsx, and inspectors-container.test.tsx (MCP `command_tool_call` → legacy-handler translation; `pushedLayers()` params unwrap). The same translator pattern is the likely fix here — or, per the tracked cards, update the assertions to the `dispatch_command` contract.

## Acceptance Criteria
- [ ] All 4 tests assert the current production contract (host-driven nav via dispatch_command, or a harness translator mirroring spatial-shadow-registry)
- [ ] `npx vitest run src/components/entity-inspector.field-vertical-nav.browser.test.tsx` green in apps/kanban-app/ui