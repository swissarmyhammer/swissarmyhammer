---
title: 'mirdan-cli: 5 new::tests failures -- NotFound OS error'
position:
  column: done
  ordinal: a4
---
5 tests in mirdan-cli/src/new.rs fail with Os { code: 2, kind: NotFound }: test_new_plugin_creates_structure (line 575), test_new_skill_creates_structure (line 480), test_new_skill_already_exists (line 532), test_new_tool_creates_structure (line 547), test_new_validator_creates_structure (line 500). Same root cause as install and list tests -- missing binary or fixture. #test-failure