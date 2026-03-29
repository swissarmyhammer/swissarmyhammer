---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffd980
title: Add tests for FieldsContext::from_yaml_sources override/error paths
---
swissarmyhammer-fields/src/context.rs:107-149\n\nCoverage: 77.8% (126/162 lines)\n\nUncovered lines: 108, 110, 112-116, 130-134, 140-141, 146-149\n\nThree untested paths in from_yaml_sources():\n1. Definition override: when a later definition has the same name as an earlier one (lines 112-116) — the old entry should be replaced\n2. Entity override: when a later entity has the same name (lines 133-134)\n3. Invalid entity YAML: the warn-and-skip path (lines 140-141)\n4. Debug log output at end (lines 146-149)\n\nTest: provide duplicate names and malformed YAML to exercise these branches. #coverage-gap