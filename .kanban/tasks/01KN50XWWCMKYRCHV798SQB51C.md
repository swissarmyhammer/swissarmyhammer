---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc980
title: '[nit] AttachmentItem missing prop interface name'
---
**File**: `kanban-app/ui/src/components/fields/displays/attachment-display.tsx:145`\n\n**What**: `AttachmentItem` uses inline `{ attachment: AttachmentMeta }` instead of a named `AttachmentItemProps` interface.\n\n**Suggestion**: Extract to `interface AttachmentItemProps { attachment: AttachmentMeta; }` for consistency with the codebase naming convention." #review-finding