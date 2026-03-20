---
position_column: done
position_ordinal: fffffffc80
title: 'Fix failing test: unified::tests::test_find_duplicates_in_file_with_file'
---
Test in swissarmyhammer-treesitter/src/unified.rs:1931 panics with `assertion failed: result.is_ok()`. The test_find_duplicates_in_file_with_file test calls a duplicate-finding function that returns an error. #test-failure