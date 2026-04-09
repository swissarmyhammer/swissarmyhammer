---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb180
title: Dispatch silently drops malformed fields/sort JSON in perspective add/update
---
dispatch.rs:427-431 and dispatch.rs:459-463\n\nThe dispatch handler for `add perspective` and `update perspective` uses `.ok()` when deserializing `fields` and `sort` params:\n```rust\nif let Some(fields) = op.get_param(\"fields\").and_then(|v| {\n    serde_json::from_value::<Vec<PerspectiveFieldEntry>>(v.clone()).ok()\n}) {\n```\n\nIf a caller passes `fields` with the wrong shape (e.g. a string instead of an array), the error is silently swallowed and the field is ignored. The caller gets a success response with no indication their fields were dropped.\n\nThis follows the existing pattern in the codebase (attachment operations do the same), so it is consistent. However, it can lead to confusing behavior where a perspective is created without the fields the caller thought they were providing.\n\nSuggestion: Log a warning when deserialization fails, similar to how changelog write failures are handled. Alternatively, return an error for malformed input.\n\nVerification: Add a test passing malformed `fields` JSON and assert the behavior (either error or logged warning)." #review-finding