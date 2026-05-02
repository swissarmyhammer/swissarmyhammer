---
assignees:
- claude-code
position_column: todo
position_ordinal: bd80
title: 'Vitest browser: 5 spatial-nav drill-in / kernel-focus tests fail (scope_chain undefined)'
---
## What

Five vitest browser-mode tests fail with `scope_chain` / dispatched moniker coming back as `undefined`, all in spatial-nav drill-in / kernel-focus paths. They reproduce on a clean checkout of HEAD with focus-debug-overlay reverted, so they are pre-existing and not caused by 01KQJHE82FPDD1YVN7RW8ZCF3T.

## Failing tests

1. `kanban-app/ui/src/components/board-view.enter-drill-in.browser.test.tsx`
   - `enter_on_focused_column_drills_into_first_card` (line 639)
     `expected undefined to be 'task:t1'` — `dispatchArgs?.scope_chain?.[0]` is undefined
   - `enter_on_focused_column_with_remembered_focus_drills_into_remembered_card` (line 710)
     `expected undefined to be 'task:t2'` — same shape

2. `kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx`
   - `enter_on_pill_field_drills_into_first_pill` (line 533)
     `expected undefined to be truthy` — pill moniker missing from `args.scope_chain`
   - `escape_from_pill_drills_back_to_field_zone` (line 690)
     `expected undefined to be truthy` — field zone moniker missing from setFocus dispatch

3. `kanban-app/ui/src/components/inspector.kernel-focus-advance.browser.test.tsx`
   - `ArrowDown from the last field stays put, kernel's focused key remains the last field` (line 518)
     `expected '/window/inspector/task:T1' to be '/window/inspector/task:T1/field:task:T1.body'` — focus did not advance into the last field zone

## Common pattern

All five expect a `scope_chain` (or last-field FQM) to contain a child moniker (`task:t1`, `task:t2`, the first pill, the last field). The code under test is dispatching `ui.setFocus` / `ui.spatial_drill_in` with a `scope_chain` that lacks that final segment. Likely a regression in either `useDispatchCommand` argument shaping, the `<FocusZone>`/scope-chain composition, or `entity-focus`'s `setFocus(moniker)` lookup.

## Repro

```
cd kanban-app/ui
pnpm exec vitest run \
  src/components/board-view.enter-drill-in.browser.test.tsx \
  src/components/entity-inspector.field-enter-drill.browser.test.tsx \
  src/components/inspector.kernel-focus-advance.browser.test.tsx
# Test Files  3 failed (3)
# Tests  5 failed | 10 passed (15)
```

Verified with `git checkout HEAD -- kanban-app/ui/src/components/focus-debug-overlay.{tsx,browser.test.tsx,layer-z.browser.test.tsx}` — same 5 failures. Pre-existing.

## Acceptance Criteria

- [ ] All 5 tests pass via the repro command above
- [ ] Full `pnpm exec vitest run` shows zero failed tests (currently 5 failed | 1880 passed | 4 skipped) #test-failure