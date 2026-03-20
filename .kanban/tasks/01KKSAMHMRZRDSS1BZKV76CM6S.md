---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffd180
title: __cancelInsert stored on view but never cleaned up
---
field-placeholder.tsx:164\n\n`(view as any).__cancelInsert = () => { cancelled = true; }` is stored on the EditorView but never called on unmount. The `cancelled` flag only prevents the rAF loop from continuing, but the cleanup reference is orphaned. The comment says \"Store cleanup on the view for the effect below\" but there is no effect that reads it.\n\nSuggestion: Either remove the __cancelInsert assignment (the rAF loop naturally terminates after 20 attempts), or add proper cleanup in the component's unmount path.