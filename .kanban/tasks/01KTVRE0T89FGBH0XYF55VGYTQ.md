---
assignees:
- claude-code
position_column: todo
position_ordinal: ee80
title: 'Pre-existing vitest browser failures: entity-inspector.field-enter-drill (2), column-view.test (2), column-view.virtualized-nav (3)'
---
## What

While implementing Card F (01KTED80H7GNF6YJJTE8MQP7CQ, 2026-06-11), blast-radius runs surfaced 7 vitest (browser/chromium) failures in files NOT covered by the existing breakage cards (01KTSQ38PF... covers board-view.enter-drill-in; 01KTSSNDN6... covers inspectable.space; 01KTS1C4EX... covers focus-scope + attachment-display):

- `apps/kanban-app/ui/src/components/entity-inspector.field-enter-drill.browser.test.tsx` (2):
  - right_from_first_pill_lands_on_second_pill
  - escape_from_pill_drills_back_to_field_zone
- `apps/kanban-app/ui/src/components/column-view.test.tsx` (2):
  - context menu dispatches task.doThisNext through the backend, not task.move
  - context menu scope chain contains the task moniker
- `apps/kanban-app/ui/src/components/column-view.virtualized-nav.browser.test.tsx` (3):
  - does not re-dispatch when the column is already at the bottom edge
  - retries at most once even when the second nav also returns stay-put
  - scrolls the column strip horizontally and re-dispatches nav from a card in the rightmost visible column

**Proven pre-existing**: the identical failing sets reproduce with the git-HEAD versions of `board-view.tsx` and `src/test/mock-command-list.ts` swapped back in (baseline runs 2026-06-11: restore HEAD copies → rerun → same failures). Card F neither causes nor fixes them.

Likely the same stale-wire-shape family as 01KTRW28RP (field-vertical-nav): nav/drill now execute host-side via `dispatch_command`, while these harnesses still count client-side `spatial_*` IPCs.

Repro: `cd apps/kanban-app/ui && npx vitest run --project browser src/components/entity-inspector.field-enter-drill.browser.test.tsx src/components/column-view.test.tsx src/components/column-view.virtualized-nav.browser.test.tsx`

## Acceptance Criteria
- [ ] Each test asserts the current production contract (host-driven nav/drill via dispatch_command, or a harness translator) without weakening the behavior pinned; all 7 green in browser mode; tsc clean.

## Workflow
- `/tdd` for any production fix. #bug #tests