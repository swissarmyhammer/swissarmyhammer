---
assignees:
- claude-code
position_column: todo
position_ordinal: b080
title: 'Fix failing test: test_use_skill_with_arguments_renders_in_output'
---
Test in `swissarmyhammer-tools/tests/integration/skill_e2e.rs:329` fails. The rendered skill output does not contain the expected arguments string. The output contains the skill JSON (description, instructions, allowed_tools) but the test expects the arguments to appear in it. #test-failure