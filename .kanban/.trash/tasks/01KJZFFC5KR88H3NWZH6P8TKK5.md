---
position_column: done
position_ordinal: l3
title: 'Fix doctest: parse_test_args in swissarmyhammer-cli/src/commands/prompt/cli.rs (line 82)'
---
Doctest fails to compile. Missing imports for parse_prompt_command, PromptCommand, and macro conflict with `matches`. Needs `use swissarmyhammer_cli::commands::prompt::cli::parse_prompt_command;` and `use swissarmyhammer_cli::commands::prompt::PromptCommand;`.