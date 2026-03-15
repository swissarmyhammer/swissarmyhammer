---
position_column: done
position_ordinal: g9
title: 'Fix test: test_execute_use_command_with_temp_config (swissarmyhammer-cli)'
---
Test panics at swissarmyhammer-cli/src/commands/model/use_command.rs:324 with 'Config should contain model key, got: ' -- the config file appears empty after writing the model setting. #test-failure