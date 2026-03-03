---
position_column: todo
position_ordinal: a1
title: 'Fix test: attachment::delete::tests::test_delete_attachment'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/delete.rs:146` fails with NotFound { resource: "attachment", id: "..." }. After adding an attachment, attempting to delete it fails because the attachment cannot be found.