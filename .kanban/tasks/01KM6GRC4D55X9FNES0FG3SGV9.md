---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff9e80
title: 'Fix builtin_yaml_files_parse: update expected command count from 21 to 23'
---
Test at swissarmyhammer-commands/src/registry.rs:416 expects 21 commands but entity.yaml now has 2 new entries (entity.archive, entity.unarchive), making the total 23. Update assertion and comment.