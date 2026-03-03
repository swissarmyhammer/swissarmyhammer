---
position_column: todo
position_ordinal: a4
title: 'Fix test: attachment::list::tests::test_list_multiple_attachments'
---
Test in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/src/attachment/list.rs:143` fails with assertion `left == right` (left: Number(0), right: 3). After adding 3 attachments, listing returns a count of 0 instead of 3.