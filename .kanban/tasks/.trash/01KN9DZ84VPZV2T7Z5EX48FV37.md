---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'Fix attachment CRUD tests: AttachmentSourceNotFound error'
---
11 tests fail with `EntityError(AttachmentSourceNotFound { path: "..." })`. The tests pass a task ID as the attachment source path instead of a real file path. Affected tests in swissarmyhammer-kanban:

- attachment::add::tests::test_add_attachment (add.rs:263)
- attachment::add::tests::test_add_attachment_auto_detect_mime
- attachment::add::tests::test_add_attachment_with_mime_type
- attachment::delete::tests::test_delete_attachment
- attachment::delete::tests::test_delete_one_of_multiple_attachments
- attachment::get::tests::test_get_attachment
- attachment::list::tests::test_list_multiple_attachments
- attachment::update::tests::test_update_attachment_mime_and_size
- attachment::update::tests::test_update_attachment_name
- dispatch::tests::dispatch_attachment_crud (dispatch.rs:1610)
- task::delete::tests::test_delete_task_removes_attachments (delete.rs:206)

#test-failure