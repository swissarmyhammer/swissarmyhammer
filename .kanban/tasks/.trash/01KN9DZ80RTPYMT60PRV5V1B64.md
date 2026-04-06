---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'Fix board-integration browser tests: ViewsProvider useState crash'
---
All 6 browser tests in `src/components/board-integration.browser.test.tsx` fail with `TypeError: Cannot read properties of null (reading 'useState')` in `ViewsProvider` (`src/lib/views-context.tsx:30`). This is a React context/hook initialization issue in the browser test harness -- likely a duplicate React instance or missing provider wrapping.\n\nFailing tests:\n1. renders the board with all columns and task cards (line 285)\n2. shows tasks in correct columns based on real data (line 298)\n3. move task between columns: entity changes on disk (line 306)\n4. drag task card on DropZone with FileDropProvider active (line 356)\n5. file drag over non-DropZone area is blocked by FileDropProvider (line 395)\n6. task drag over non-DropZone area is NOT blocked (regression test) (line 409)\n\nFile: `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban-perspective/kanban-app/ui/src/components/board-integration.browser.test.tsx` #test-failure