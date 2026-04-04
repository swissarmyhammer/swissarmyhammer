---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd480
title: Fix mention-pill.test.tsx failures (2 tests)
---
Two failures in `src/components/mention-pill.test.tsx`:\n- right-click shows context menu with ui.inspect and task.untag for tags\n- task.untag not available when taskId is undefined\n\nLikely broken by recent context menu / command refactors.\n\n#test-failure #test-failure