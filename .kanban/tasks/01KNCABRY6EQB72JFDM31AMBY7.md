---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe280
title: Fix board-integration.browser.test.tsx failures (6 tests)
---
All 6 browser/chromium tests failing in `src/components/board-integration.browser.test.tsx`:\n- renders the board with all columns and task cards\n- shows tasks in correct columns based on real data\n- move task between columns: entity changes on disk\n- drag task card on DropZone with FileDropProvider active\n- file drag over non-DropZone area is blocked by FileDropProvider\n- task drag over non-DropZone area is NOT blocked (regression test)\n\nMay be environment/chromium setup issue or broken by recent refactors.\n\n#test-failure #test-failure