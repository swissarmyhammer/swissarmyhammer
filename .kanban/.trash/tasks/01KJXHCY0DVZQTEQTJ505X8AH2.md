---
position_column: done
position_ordinal: d8
title: 'Fix test: test_execute_use_command_with_temp_config'
---
Test in swissarmyhammer-cli/src/commands/model/use_command.rs:324 panics with: Config should contain model key, got: (empty). The test sets a model via use_command but the config file does not contain the expected model key afterward. #test-failure