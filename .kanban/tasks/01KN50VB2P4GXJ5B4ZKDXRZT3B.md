---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc680
title: '[warning] AttachmentDisplay and AttachmentListDisplay use anonymous inline prop types'
---
**File**: `kanban-app/ui/src/components/fields/displays/attachment-display.tsx:229-236` and `:284-291`\n\n**What**: Both `AttachmentDisplay` and `AttachmentListDisplay` use anonymous inline prop types `{ value: unknown; mode: \"compact\" | \"full\"; onCommit?: ... }` instead of named interfaces.\n\n**Why**: Same guideline violation as AttachmentItemInner. These are public display components that should have named, documented prop interfaces.\n\n**Suggestion**: Create `interface AttachmentDisplayProps` and `interface AttachmentListDisplayProps`." #review-finding