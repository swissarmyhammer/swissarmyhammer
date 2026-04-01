---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
title: 'Fix test: test_init_board_idempotent_gitattributes'
---
Test `board::init::tests::test_init_board_idempotent_gitattributes` panics at `init.rs:475` with `No such file or directory`. Same root cause as the other gitattributes tests -- the file is not being created before the read. #test-failure