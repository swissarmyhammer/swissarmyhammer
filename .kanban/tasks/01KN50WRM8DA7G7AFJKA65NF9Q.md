---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffba80
title: '[warning] process_attachment_field silently drops non-string, non-object array elements'
---
**File**: `swissarmyhammer-entity/src/context.rs:1783`\n\n**What**: In the `multiple` branch of `process_attachment_field`, the `_ => {}` match arm silently drops array elements that are neither `Value::String` nor `Value::Object`. For example, a `Value::Number` or `Value::Bool` would be silently removed from the attachments array.\n\n**Why**: Silent data loss. If a misconfigured client sends `[\"file.txt\", 42, \"other.txt\"]`, the `42` is silently dropped and the entity is saved with only two attachments. An error or at minimum a tracing::warn would prevent debugging confusion.\n\n**Suggestion**: Add `_ => { tracing::warn!(field = field_name, \"skipping non-string/non-object value in attachment array\"); }` or return an error for unexpected types." #review-finding