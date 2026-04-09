---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc180
title: Add tests for CommandsRegistry YAML loading and merge
---
swissarmyhammer-commands/src/registry.rs\n\nCoverage: 53.9% (41/76 lines)\n\nUncovered functions:\n- from_yaml_sources (3 lines: 41-48)\n- merge_yaml_sources (6 lines: 59-68)\n- all_commands (2 lines: 80-81)\n- merge_yaml_value (10 lines: 106-137) -- recursive YAML merge logic\n- load_yaml_dir (12 lines: 202-220)\n\nWhat to test: Create temp YAML files with command definitions, load via from_yaml_sources, verify commands are registered. Test merge_yaml_value merges mappings recursively and arrays concatenate. Test load_yaml_dir discovers and loads all .yaml files. #coverage-gap