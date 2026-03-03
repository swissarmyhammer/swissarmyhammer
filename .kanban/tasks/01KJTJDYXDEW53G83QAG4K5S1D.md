---
position_column: todo
position_ordinal: a2
title: 'Fix test: attachment::delete::tests::test_delete_one_of_multiple_attachments'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/delete.rs:228` fails with NotFound { resource: "attachment", id: "..." }. After adding multiple attachments, attempting to delete one fails because it cannot be found.