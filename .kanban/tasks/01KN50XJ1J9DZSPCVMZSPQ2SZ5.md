---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc380
title: '[nit] normalizeAttachments uses unsafe cast without validation'
---
**File**: `kanban-app/ui/src/components/fields/editors/attachment-editor.tsx:34-38`\n\n**What**: `normalizeAttachments` casts the input with `as Array<AttachmentMeta | string>` and `as AttachmentMeta | string` without validating that the runtime value actually matches. If the backend sends an unexpected shape (e.g. a bare number), this would propagate without error.\n\n**Suggestion**: Add minimal runtime validation -- at least check that array elements are strings or objects with an `id` property. The function already handles null/undefined; extending it to filter invalid elements would make it robust." #review-finding