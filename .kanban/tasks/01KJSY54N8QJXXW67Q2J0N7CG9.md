---
position_column: done
position_ordinal: g9
title: DeleteTask leaves orphaned attachment entities
---
**Done.** DeleteTask now trashes attachment entities before deleting the task.\n\n- [x] Read task's attachments list before deletion\n- [x] Delete each attachment entity via ectx.delete()\n- [x] Test: create task with attachment, delete task, verify attachment gone