---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd880
title: 'Fix failing test: test_use_skill_with_arguments_renders_in_output'
---
Test `integration::skill_e2e::test_use_skill_with_arguments_renders_in_output` in `swissarmyhammer-tools` fails. The test at `swissarmyhammer-tools/tests/integration/skill_e2e.rs:329` asserts that rendered skill output contains the arguments string, but the actual output does not include it. The rendered output contains skill metadata (description, instructions, allowed_tools) but is missing the expected arguments rendering. #test-failure