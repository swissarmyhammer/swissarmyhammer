---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffca80
title: Add tests for YamlExpander::parse_and_expand
---
src/yaml_expander.rs:264-268\n\nCoverage: 0%\n\nUncovered lines: 266, 268\n\n```rust\npub fn parse_and_expand(&self, yaml: &str) -> Result<serde_yaml_ng::Value>\n```\n\nConvenience method that parses a YAML string then expands includes. Test both the success path and the YAML parse error path. #coverage-gap