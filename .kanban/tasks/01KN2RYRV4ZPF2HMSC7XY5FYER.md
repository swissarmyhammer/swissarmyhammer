---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9980
title: 'handleCommit double-writes: flushSave() and updateField() can both fire the same value'
---
**File:** `kanban-app/ui/src/components/fields/field.tsx` lines 131-138\n**Severity:** warning\n\nIn `handleCommit`, `flushSave()` fires any pending debounced save, then `updateField()` is called unconditionally with `newValue`. If the debounced value equals `newValue` (the common case -- user types, then presses Enter), this sends two backend writes for the same value.\n\nFix: either (a) `cancel()` instead of `flush()` since the explicit commit supersedes the debounce, or (b) check whether `flushSave()` already sent the value before calling `updateField()` again. Option (a) is simpler and correct: the commit value is authoritative, so the pending debounced value should be discarded. #review-finding