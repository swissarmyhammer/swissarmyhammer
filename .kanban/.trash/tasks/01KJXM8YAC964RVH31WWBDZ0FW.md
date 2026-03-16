---
position_column: done
position_ordinal: f3
title: 'Fix swissarmyhammer-config test: test_with_template_vars YAML file not found'
---
The test `template_context::tests::test_with_template_vars` in swissarmyhammer-config intermittently panics with LoadError: "Failed to read YAML file: No such file or directory" for a temp path `.swissarmyhammer/sah.yaml`. Race condition with CWD or temp directory cleanup in parallel test execution.