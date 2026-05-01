---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffff080
title: 'field-renderer.tsx: hardcoded computed editability check'
---
**File:** `kanban-app/ui/src/components/field-renderer.tsx:36`\n\n```ts\nconst editable = field.editor !== \"none\" && field.type.kind !== \"computed\";\n```\n\nHardcodes `field.type.kind !== \"computed\"` to block editing of computed fields, ignoring the YAML-configured `editor` property. Should use `resolveEditor(field) !== \"none\"` — same fix already applied to entity-inspector.tsx. #field-special-case