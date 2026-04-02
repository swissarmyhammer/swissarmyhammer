---
assignees:
- claude-code
position_column: todo
position_ordinal: '8780'
title: 'Fix drop-zone.test.tsx: onDrop callback never fires in 2 tests'
---
Two tests in src/components/drop-zone.test.tsx fail with 'expected vi.fn() to be called 1 times, but got 0 times':\n1. 'fires onDrop with descriptor when drop event occurs' (line 85)\n2. 'empty-column zone fires onDrop (no before/after in descriptor)' (line 112)\nThe onDrop mock is never called when fireEvent.drop is triggered. The DropZone component likely changed how it handles drop events. #test-failure