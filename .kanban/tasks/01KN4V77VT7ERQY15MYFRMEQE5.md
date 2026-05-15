---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffa80
title: ChangelogEntry stores full before/after text AND patches -- redundant storage
---
**swissarmyhammer-store/src/changelog.rs:32-51**\n\n`ChangelogEntry` stores `before: Option<String>`, `after: Option<String>`, `forward_patch: Option<String>`, and `reverse_patch: Option<String>`. For large items, this means the changelog stores 4x the data: full old text, full new text, forward diff, and reverse diff.\n\nThe `undo()` and `redo()` methods in `handle.rs` use `before`/`after` directly for the fast path and only use three-way merge for the concurrent-edit case. The `forward_patch`/`reverse_patch` fields are computed (line 82-83 in handle.rs) but never actually consumed by any code path.\n\n**Severity: warning**\n\n**Suggestion:** Either:\n1. Remove `forward_patch`/`reverse_patch` since they are never used (simplify).\n2. Use patches instead of full text for undo/redo to reduce storage (but this changes the merge strategy).\n3. At minimum, stop storing full `before`/`after` text for Update operations and rely on patches.\n\n**Subtasks:**\n- [ ] Audit all consumers of `forward_patch` and `reverse_patch`\n- [ ] Decide which representation to keep\n- [ ] Remove the unused fields or the unused full-text fields\n- [ ] Verify undo/redo still works after the change" #review-finding