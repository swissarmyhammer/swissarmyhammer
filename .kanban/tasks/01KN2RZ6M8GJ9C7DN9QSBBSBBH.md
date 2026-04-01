---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffaa80
title: useDebouncedSave silently swallows updateField errors
---
**File:** `kanban-app/ui/src/lib/use-debounced-save.ts` lines 92 and 113\n**Severity:** nit\n\nBoth `.catch(() => {})` calls in the hook silently swallow errors from `updateField`. Since this is autosave (user did not explicitly commit), a silent failure means data loss without any user feedback. Consider at minimum `console.warn`-ing the error so it surfaces in the unified log for debugging. #review-finding