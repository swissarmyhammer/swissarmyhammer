---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc280
title: '[warning] AttachmentItemInner anonymous prop types violate named-interface guideline'
---
**File**: `kanban-app/ui/src/components/fields/displays/attachment-display.tsx:196-204`\n\n**What**: `AttachmentItemInner` uses an anonymous inline object type for its props: `{ attachment: AttachmentMeta; scopeChain: string[]; Icon: ComponentType<...> }`. The JS_TS_REVIEW guideline requires: 'Named prop interfaces. Every component gets an `interface FooProps` co-located above it. No anonymous inline object types.'\n\n**Suggestion**: Extract into `interface AttachmentItemInnerProps { attachment: AttachmentMeta; scopeChain: string[]; Icon: ComponentType<{ className?: string; size?: number }>; }`" #review-finding