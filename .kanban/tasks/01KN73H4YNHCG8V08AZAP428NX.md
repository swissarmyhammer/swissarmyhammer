---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff9280
title: Add tests for FileProvider YAML null handling and unsupported extension
---
swissarmyhammer-config/src/provider.rs:40-110\n\nCoverage: 77.2% (71/92 lines)\n\nUncovered lines: 43-45, 57, 59, 61, 67, 101-103, 108-109\n\n```rust\nfn validate_and_merge_yaml(&self, figment: Figment) -> ConfigurationResult<Figment>\nfn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> // unsupported ext branch\nfn metadata(&self) -> Metadata\n```\n\nvalidate_and_merge_yaml reads YAML content, checks for null/empty files, and skips them gracefully. The load_into match has an unsupported extension error branch. Test: load a YAML file that parses to null (empty file), load a file with an unsupported extension (e.g. .ini), and verify the metadata() method returns expected name. #Coverage_Gap