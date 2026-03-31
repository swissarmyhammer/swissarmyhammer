---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff9080
title: Add tests for latest_value_timestamps (yaml.rs)
---
swissarmyhammer-merge/src/yaml.rs:113-133\n\n`fn latest_value_timestamps(entries: &[ChangelogEntry]) -> HashMap<String, (String, String)>`\n\nBuilds a map of field name to (most recent value, timestamp). Tested only indirectly through conflict resolution tests. Needs direct tests for:\n- Empty entries list\n- Multiple entries touching the same field (later timestamp wins)\n- Multiple fields across entries\n- Entries without `new_value` (should be ignored) #coverage-gap