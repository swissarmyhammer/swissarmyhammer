---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffd880
title: Add tests for Pretty Display/Debug impls
---
swissarmyhammer-common/src/lib.rs:110-124\n\nCoverage: 0% (0/8 lines)\n\nUncovered lines: 111-114, 120-123\n\nFunctions: impl Display for Pretty<T>::fmt, impl Debug for Pretty<T>::fmt\n\nPretty is a wrapper for pretty-printing types as YAML in logs. Both fmt impls attempt serde_yaml_ng::to_string and fall back to Debug formatting on error. Test both paths: successful YAML serialization and the fallback path when serialization fails (e.g., a type that implements Debug but fails serde). #coverage-gap