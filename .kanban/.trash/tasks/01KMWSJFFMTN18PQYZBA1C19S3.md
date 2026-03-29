---
assignees:
- claude-code
position_column: todo
position_ordinal: 8d80
title: 'Fix test: test_init_board_registers_git_config'
---
Test `board::init::tests::test_init_board_registers_git_config` panics at `init.rs:412` with assertion `.git/config should contain [merge "kanban-jsonl"]`. The merge driver registration is not writing to git config correctly. #test-failure