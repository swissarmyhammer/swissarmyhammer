---
assignees:
- claude-code
position_column: todo
position_ordinal: af80
title: Fix mention-pill.test.tsx failures (2 tests)
---
Two failures in src/components/mention-pill.test.tsx:\n1. right-click shows context menu with ui.inspect and task.untag for tags\n2. task.untag not available when taskId is undefined\n\n#test-failure #test-failure