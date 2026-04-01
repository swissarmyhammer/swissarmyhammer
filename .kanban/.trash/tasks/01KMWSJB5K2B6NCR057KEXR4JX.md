---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
title: 'Fix test: test_register_merge_drivers_preserves_existing_gitattributes'
---
Test `board::init::tests::test_register_merge_drivers_preserves_existing_gitattributes` panics at `init.rs:629` with `No such file or directory`. The test tries to unwrap a file read that fails because the gitattributes file does not exist in the temp dir. #test-failure