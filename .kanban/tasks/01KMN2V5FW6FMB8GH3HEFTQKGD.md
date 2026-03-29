---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffbf80
title: 'Fix failing tests: MultiSelectEditor - reference field tokens, tag field slugs and display (5 tests)'
---
File: /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/fields/editors/multi-select-editor.test.tsx\n\nFailing tests:\n- MultiSelectEditor > reference field (assignees) > shows existing selections as prefixed tokens in the doc\n- MultiSelectEditor > reference field (assignees) > multiple selections appear as separate tokens in the doc\n- MultiSelectEditor > reference field (assignees) > deleting a token from the doc removes it from committed value\n- MultiSelectEditor > computed tag field > Enter commits tag slugs via onCommit (received \"tag-bug\" instead of \"bug\")\n- MultiSelectEditor > computed tag field > deleting a tag token from the doc removes it from committed value (doc contains \"tag-bug\" instead of \"#bug\") #test-failure