---
position_column: done
position_ordinal: y4
title: 'TS6133: Unused declarations in multi-select-editor.tsx (onCancel, entity, addItem)'
---
TypeScript --noEmit reports 3 errors in src/components/fields/editors/multi-select-editor.tsx:\n- Line 43: 'onCancel' is declared but its value is never read\n- Line 44: 'entity' is declared but its value is never read\n- Line 115: 'addItem' is declared but its value is never read #test-failure