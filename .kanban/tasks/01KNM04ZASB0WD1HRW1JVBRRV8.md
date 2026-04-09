---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8b80
title: 'Fix: ReferenceError invokeFocusChange is not defined in entity-focus-context.tsx:182'
---
All 6 unhandled errors during vitest run are the same root cause: `invokeFocusChange is not defined` at `handleWindowFocus` in `src/lib/entity-focus-context.tsx:182`. This surfaces in 6 test files:\n- autosave-focus.browser.test.tsx\n- editor-save.test.tsx\n- entity-inspector.test.tsx\n- entity-card.test.tsx\n- command-palette.test.tsx\n- app-shell.test.tsx\n\nThe variable `invokeFocusChange` is referenced but not in scope at line 182. #test-failure