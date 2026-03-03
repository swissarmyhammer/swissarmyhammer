---
position_column: todo
position_ordinal: a5
title: 'Fix test: attachment::update::tests::test_update_attachment_mime_and_size'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/update.rs:211` fails with NotFound { resource: "attachment", id: "..." }. After adding an attachment, attempting to update its MIME type and size fails because the attachment cannot be found.