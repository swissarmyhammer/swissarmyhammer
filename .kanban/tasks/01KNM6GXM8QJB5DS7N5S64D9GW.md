---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8c80
title: 'Fix ReferenceError: invokeFocusChange is not defined in entity-focus-context.tsx:182'
---
All 6 unhandled errors in the vitest suite share the same root cause: `invokeFocusChange` is referenced at line 182 of `src/lib/entity-focus-context.tsx` inside `handleWindowFocus` but is not defined. This causes unhandled ReferenceErrors in 6 test files:\n\n1. src/components/fields/autosave-focus.browser.test.tsx\n2. src/components/fields/editors/editor-save.test.tsx\n3. src/components/entity-inspector.test.tsx\n4. src/components/entity-card.test.tsx\n5. src/components/command-palette.test.tsx\n6. src/components/app-shell.test.tsx\n\nThe error surfaces when CodeMirror calls `EditorView.focus()` which triggers `handleWindowFocus`. #test-failure