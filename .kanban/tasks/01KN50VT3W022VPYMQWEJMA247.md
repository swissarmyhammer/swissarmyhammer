---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc480
title: '[warning] No test for attachment field type round-trip through YAML on the entity definition level'
---
**File**: `swissarmyhammer-kanban/builtin/fields/definitions/attachments.yaml`\n\n**What**: The `attachments.yaml` field definition uses `kind: attachment` with `max_bytes` and `multiple`. The `FieldType::Attachment` variant has YAML round-trip tests in `types.rs`, but there is no integration test that loads the actual builtin YAML file and verifies it parses correctly with `from_yaml_sources()`.\n\n**Why**: If the YAML file has a typo or schema mismatch, it would fail silently at runtime (the `from_yaml_sources` code logs a warning and skips). An integration test that loads the actual builtin definitions and asserts the attachment field is present would catch this at CI time.\n\n**Suggestion**: Add a test in `defaults.rs` or `context.rs` that calls `builtin_field_definitions()`, builds a `FieldsContext`, and asserts `get_field_by_name(\"attachments\")` returns `Some` with `FieldType::Attachment { multiple: true, .. }`." #review-finding