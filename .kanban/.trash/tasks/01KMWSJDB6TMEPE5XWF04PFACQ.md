---
assignees:
- claude-code
position_column: todo
position_ordinal: 8c80
title: 'Fix test: test_init_board_creates_gitattributes'
---
Test `board::init::tests::test_init_board_creates_gitattributes` panics at `init.rs:432` with `No such file or directory`. The board init flow fails to create or find the `.gitattributes` file. #test-failure