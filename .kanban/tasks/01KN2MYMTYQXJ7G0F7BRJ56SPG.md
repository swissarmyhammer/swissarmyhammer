---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: builtin_yaml_files_parse test only covers 4 of 6 YAML sources
---
swissarmyhammer-commands/src/registry.rs:399-430\n\nThe `builtin_yaml_files_parse` test loads app, entity, ui, and settings YAML but omits `file.yaml` and `drag.yaml` which are present in `builtin_yaml_sources()`. The asserted count of 34 only covers the 4 loaded files. If commands are added to file.yaml or drag.yaml, this test won't catch count drift.\n\nSuggestion: Load all 6 builtin sources using `builtin_yaml_sources()` directly, and update the expected count to match the full set." #review-finding