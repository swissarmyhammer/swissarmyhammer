---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd080
title: Add tests for YamlExpander::list_names and Default impl
---
src/yaml_expander.rs:58-61, 140-141\n\nCoverage: 0% for these methods\n\nUncovered lines: 59-60, 140-141\n\n```rust\nimpl Default for YamlExpander { fn default() -> Self }\npub fn list_names(&self) -> Vec<&String>\n```\n\nTrivial methods but untested. Add tests that verify Default::default() creates an empty expander, and list_names returns the correct set of include names after adding builtins. #coverage-gap