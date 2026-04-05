---
assignees:
- claude-code
position_column: todo
position_ordinal: af80
title: 'Fix failing test: test_use_skill_with_arguments_renders_in_output'
---
Test `integration::skill_e2e::test_use_skill_with_arguments_renders_in_output` in `swissarmyhammer-tools` fails. The rendered skill output does not contain the expected arguments string. The test panics at `swissarmyhammer-tools/tests/integration/skill_e2e.rs:329`. #test-failure