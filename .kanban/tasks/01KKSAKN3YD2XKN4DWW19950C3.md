---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffc880
title: Remove debug console.log calls before merge
---
cm-submit-cancel.ts:69,95 and field-placeholder.tsx:135 and quick-capture.tsx:148\n\nMultiple `console.log` debug statements left in production code. These were essential during development but should be removed or gated behind a debug flag before merging to main.\n\n- `[cm-submit-cancel] vim Enter capture, submitting` (line 69)\n- `[cm-submit-cancel] vim Escape in normal mode, cancelling` (line 95)\n- `[field-placeholder] semanticCancel → onCancel()` (line 135)\n- `[quick-capture] hideWindow called` (line 148)\n\nWith the oslog pipeline now working, these flood the log stream with noise.