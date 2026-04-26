---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffd280
title: Add tests for WebFetchRequest deserialization validation
---
src/types.rs:56-114\n\nCoverage: 3.7% (1/27 lines)\n\nUncovered lines: 57, 72, 78-83, 86, 94-99, 102, 106-111\n\n```rust\nimpl Deserialize for WebFetchRequest\n```\n\nCustom deserializer with validation for timeout (1-120s) and max_content_length (1024-10MB). Needs tests for:\n1. Valid deserialization with all fields\n2. Timeout out of range (0, 121) → error\n3. max_content_length out of range (0, 11MB) → error\n4. Valid edge cases (timeout=1, timeout=120, etc.) #coverage-gap