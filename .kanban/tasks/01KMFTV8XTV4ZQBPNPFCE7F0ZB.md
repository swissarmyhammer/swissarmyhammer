---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffef80
title: 'field-renderer.tsx: fix hardcoded computed editability check'
---
Fix hardcoded computed editability check in `kanban-app/ui/src/components/field-renderer.tsx:36`.

Current code:
```ts
const editable = field.editor !== "none" && field.type.kind !== "computed";
```

This hardcodes `field.type.kind !== "computed"` to block editing of computed fields, ignoring the YAML-configured `editor` property. It should use `resolveEditor(field) !== "none"` — the same fix already applied to `entity-inspector.tsx`.

Steps:
1. Read field-renderer.tsx to see current code and imports
2. Read entity-inspector.tsx to see how resolveEditor is used (reference implementation)
3. Find where resolveEditor is defined
4. Replace hardcoded check with resolveEditor(field) !== "none"
5. Add import for resolveEditor if not already imported
6. Run TypeScript type checker
7. Run tests