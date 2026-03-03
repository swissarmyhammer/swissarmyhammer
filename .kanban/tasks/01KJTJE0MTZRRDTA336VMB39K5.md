---
position_column: todo
position_ordinal: a3
title: 'Fix test: attachment::get::tests::test_get_attachment'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/get.rs:110` fails with NotFound { resource: "attachment", id: "..." }. After adding an attachment, attempting to retrieve it by ID fails with a NotFound error.