---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffda80
title: UndoEntryId::Default generates a new unique ID -- surprising semantics
---
**swissarmyhammer-store/src/id.rs:32-36**\n\n```rust\nimpl Default for UndoEntryId {\n    fn default() -> Self {\n        Self::new()\n    }\n}\n```\n\n`Default::default()` is expected to return a deterministic, zero-like value. This impl generates a brand new random ULID each time, which is surprising. Code using `#[serde(default)]` or `Option<UndoEntryId>::unwrap_or_default()` would silently generate new IDs instead of getting a sentinel value.\n\n**Severity: nit**\n\n**Suggestion:** Remove the `Default` impl entirely. If a sentinel/nil value is needed, add an explicit `UndoEntryId::nil()` using `Ulid::nil()`.\n\n**Subtasks:**\n- [ ] Remove `Default` impl from `UndoEntryId`\n- [ ] Search codebase for `.unwrap_or_default()` or `#[serde(default)]` usage with this type\n- [ ] Verify compilation across workspace" #review-finding