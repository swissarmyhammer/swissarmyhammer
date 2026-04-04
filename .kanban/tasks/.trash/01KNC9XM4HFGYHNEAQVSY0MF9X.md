---
assignees:
- claude-code
position_column: todo
position_ordinal: b180
title: Fix editor-save.test.tsx failures (2 tests)
---
Two failures in src/components/fields/editors/editor-save.test.tsx:\n- field: depends_on (editor: multi-select) > mode: compact > keymap: vim > exit: Enter\n- field: depends_on (editor: multi-select) > mode: full > keymap: vim > exit: Enter\n\n#test-failure #test-failure