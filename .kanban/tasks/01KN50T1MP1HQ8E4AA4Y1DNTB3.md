---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffbf80
title: '[warning] isMultiple() in attachment-editor.tsx casts field.type to Record<string, unknown>'
---
**File**: `kanban-app/ui/src/components/fields/editors/attachment-editor.tsx:47`\n\n**What**: `(field.type as Record<string, unknown>).multiple !== false` violates the JS_TS_REVIEW guideline: 'No `as Record<string, unknown>` casts on field types. If you need a property from `field.type`, it should be surfaced as a top-level field property or handled by the backend's `effective_*()` methods before reaching the frontend.'\n\n**Why**: This is a hardcoded structural assumption about `field.type`. If `FieldType` gains a new variant or the shape changes, this cast silently breaks. The `multiple` flag should either be a top-level field property on `FieldDef` (backend sends it) or the backend should send an `effective_multiple` property.\n\n**Suggestion**: Add a `multiple` (or `attachment_multiple`) property to FieldDef in the backend that the YAML schema sets, so the editor reads `field.multiple` directly without casting." #review-finding