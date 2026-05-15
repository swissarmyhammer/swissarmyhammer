---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffef80
title: Add tests for merge_yaml ParseFailure paths
---
swissarmyhammer-merge/src/yaml.rs:152-281\n\n`pub fn merge_yaml(base, ours, theirs, opts) -> Result<String, MergeError>`\n\nThe happy path and conflict paths are well tested (10 tests). Missing coverage:\n- `MergeError::ParseFailure` when base is invalid YAML (not a mapping)\n- `MergeError::ParseFailure` when ours is invalid YAML\n- `MergeError::ParseFailure` when theirs is invalid YAML\n- Non-string YAML keys (the `format!(\"{k:?}\")` branch at line 175)\n- YAML Null document treated as empty mapping (line 181) #coverage-gap