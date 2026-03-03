---
position_column: todo
position_ordinal: a6
title: 'Fix test: attachment::update::tests::test_update_attachment_name'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/update.rs:181` fails with NotFound { resource: "attachment", id: "..." }. After adding an attachment, attempting to update its name fails because the attachment cannot be found.