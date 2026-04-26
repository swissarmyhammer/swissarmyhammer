---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffd080
title: Test entity store.rs YAML serialization error path (95.0%)
---
**File**: `swissarmyhammer-entity/src/store.rs` (95.0% -- 38/40 lines)\n\n**What**: Two uncovered lines:\n- L134: Error propagation from `serde_yaml_ng::to_string()` in `serialize()` for MD+YAML format\n- L167: Error mapping from `serde_yaml_ng::from_str()` in `deserialize()` for malformed YAML within valid frontmatter delimiters\n\n**Acceptance criteria**: Coverage at or above 97%\n\n**Tests to add**:\n- Test `deserialize()` with valid `---` delimiters but garbage YAML in the frontmatter section (triggers L167)\n- The serialization error at L134 is very hard to trigger since `serde_yaml_ng::to_string` rarely fails on valid `Value::Object` -- may be acceptable to skip" #coverage-gap