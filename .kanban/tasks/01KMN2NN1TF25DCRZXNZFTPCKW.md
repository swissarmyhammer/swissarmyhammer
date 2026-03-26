---
assignees:
- claude-code
position_column: todo
position_ordinal: 7a80
title: 'Fix multi-select-editor.test.tsx failures (5 tests): missing @ and # prefixes in CM6 doc'
---
MultiSelectEditor is not prepending the sigil prefix to tokens in the CodeMirror document. Reference fields should use @ prefix, tag fields should use # prefix.\n\nFailing tests:\n- reference field > shows existing selections as prefixed tokens: doc contains 'alice ' not '@alice'\n- reference field > multiple selections appear as separate tokens: 'alice bob ' not '@alice'/'@bob'\n- reference field > deleting a token from the doc removes it: indexOf('@alice') returns -1\n- computed tag field > Enter commits tag slugs via onCommit: onCommit called with ['tag-bug'] instead of ['bug']\n- computed tag field > deleting a tag token: doc contains 'tag-bug tag-feat ' not '#bug'/'#feature'\n\nFile: `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/fields/editors/multi-select-editor.test.tsx`"