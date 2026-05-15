---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8180
title: Add tests for CommandsRegistry::merge_yaml_sources
---
registry.rs:59-72\n\n`merge_yaml_sources(&mut self, sources)` — merges additional YAML into an existing registry.\n\nCurrently only tested indirectly via from_yaml_sources override tests. Needs direct tests:\n1. Build registry, then merge_yaml_sources to add new commands\n2. merge_yaml_sources overrides existing command fields\n3. Invalid YAML in merge is skipped without affecting existing entries