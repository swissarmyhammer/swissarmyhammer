---
position_column: done
position_ordinal: d7
title: 'Fix failing test: test_execute_use_command_with_temp_config'
---
Test `commands::model::use_command::tests::test_execute_use_command_with_temp_config` in swissarmyhammer-cli fails. The test sets a model via the use command, which prints success, but then the config file reads back as `null` for the model key. Panic message: "Config should contain model key, got: null". File: /Users/wballard/github/swissarmyhammer/swissarmyhammer-tools/swissarmyhammer-cli/src/commands/model/use_command.rs:324 #test-failure