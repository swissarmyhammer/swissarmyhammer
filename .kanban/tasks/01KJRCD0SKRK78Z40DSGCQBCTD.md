---
title: 'Fix test_tag_file_based_storage: board.yaml not found at line 56'
position:
  column: todo
  ordinal: c6
---
Test `test_tag_file_based_storage` in `swissarmyhammer-kanban/tests/integration_tag_storage.rs` panics at line 56 with "No such file or directory" when calling `std::fs::read_to_string(kanban_dir.join("board.yaml")).unwrap()`. The board.yaml file is not being created in the expected location after `InitBoard` runs. This may indicate that the storage format or directory layout changed but the test was not updated to match. #test-failure