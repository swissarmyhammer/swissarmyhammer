---
position_column: done
position_ordinal: ffffffa080
title: 'FAIL: MultiSelectEditor > remove button removes item from selection - no .bg-muted pills found (0 instead of 2)'
---
File: src/components/fields/editors/multi-select-editor.test.tsx:285\nTest expects 2 pills with .bg-muted class but finds 0. Same root cause as the other two failures - actor pill rendering does not use .bg-muted class. #test-failure