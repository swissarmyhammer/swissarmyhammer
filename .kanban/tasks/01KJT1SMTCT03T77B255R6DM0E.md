---
position_column: done
position_ordinal: a1
title: 'Fix failing test: test_model_list_with_invalid_model_files'
---
Test `integration::model_commands::test_model_list_with_invalid_model_files` in `swissarmyhammer-cli/tests/integration/model_commands.rs` (line 586) panics with assertion "Should list valid model". The test expects `stdout.contains("valid-agent")` to be true after running `model list`, but the output does not contain the expected string. This is in the `swissarmyhammer-cli` package. #test-failure