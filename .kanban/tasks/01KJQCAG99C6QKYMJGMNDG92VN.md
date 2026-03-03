---
title: 'mirdan-cli: 2 list::tests failures -- NotFound OS error'
position:
  column: done
  ordinal: a3
---
2 tests in mirdan-cli/src/list.rs fail with Os { code: 2, kind: NotFound }: test_run_list_agent_filter_suppresses_validators (line 390), test_run_list_no_filter_shows_validators (line 414). Same root cause as install tests -- missing binary or fixture. #test-failure