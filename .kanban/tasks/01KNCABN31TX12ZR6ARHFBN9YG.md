---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe180
title: Fix editor-save.test.tsx failures (2 tests)
---
Two failures in `src/components/fields/editors/editor-save.test.tsx`:\n- field: depends_on (multi-select) > mode: compact > keymap: vim > exit: Enter\n- field: depends_on (multi-select) > mode: full > keymap: vim > exit: Enter\n\nVim keymap + Enter interaction issue specific to depends_on multi-select field.\n\n#test-failure #test-failure