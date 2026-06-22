---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvqrra3r7epyatrvapf0ek57
  text: |-
    Picked up. Repro: after `npx playwright install chromium` (browser was missing), the repro command showed only 2/7 genuinely failing — both in column-view.test.tsx ("...dispatches task.doThisNext through the backend, not task.move" and "...scope chain contains the task moniker"). The entity-inspector.field-enter-drill (2) and column-view.virtualized-nav (3) tests were already green against current production once the browser was installed (their prior failures were the missing-playwright unhandled-error, not stale wire-shape).

    Root cause (the 2 real failures): production `openContextMenu` in src/lib/context-menu.ts no longer reads the client-side `useCommandList` hook. It fetches the registry via `callCommandTool("list command")` → invoke("command_tool_call", {module:"commands", op:"list command", ...}) and pops via `callMcpTool("window","show context menu",...)` → invoke("command_tool_call", {module:"window", op:"show context menu", ...}). The tests' mockInvoke returned "ok" for command_tool_call, so result.commands was undefined → 0 matching items → "show context menu" never fired → showCall undefined.

    Fix (test-harness only): added helper `serveContextMenuCommandTool(args)` that returns `{ ok: true, commands: mockRegistry }` for op "list command" and undefined otherwise (matching the real fire-and-forget bridge), and routed `cmd === "command_tool_call"` through it in both tests. The assertions (already on the production wire shape: c[0]==="command_tool_call" && c[1].op==="show context menu") now pass without weakening behavior pinned (task.doThisNext surfaced, task.move NOT dispatched, scope_chain contains task:tN).

    No production code changed.
  timestamp: 2026-06-22T13:36:40.568250+00:00
- actor: claude-code
  id: 01kvqrtz51st31p1yv656wpam9
  text: |-
    Gates green. Repro `npx vitest run --project browser src/components/entity-inspector.field-enter-drill.browser.test.tsx src/components/column-view.test.tsx src/components/column-view.virtualized-nav.browser.test.tsx` → "Test Files 3 passed (3); Tests 24 passed (24)" (all 7 originally-named tests included). `npx tsc --noEmit` in apps/kanban-app/ui → exit 0, clean.

    double-check (adversarial) verdict: PASS — change is test-only (only column-view.test.tsx modified under src; context-menu.ts and mcp-transport.ts untouched), exercises the real production openContextMenu end-to-end, and pins all three behaviors without weakening (task.doThisNext surfaced via live wire, task.move never dispatched, scope_chain contains task moniker). Non-blocking note: the vi.mock("@/hooks/use-command-list",...) at the top of the file is now dead weight for the context-menu path (production only imports types from it) — harmless, removal left out of scope.

    Moving to review.
  timestamp: 2026-06-22T13:38:07.649145+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffd780
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