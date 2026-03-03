---
position_column: todo
position_ordinal: a7
title: 'Fix test: task::delete::tests::test_delete_task_removes_attachments'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/task/delete.rs:224` fails with assertion `ectx.read("attachment", &att_id).await.is_err()`. After deleting a task, its attachments are not being cleaned up properly -- the attachment entity still exists when it should have been removed.