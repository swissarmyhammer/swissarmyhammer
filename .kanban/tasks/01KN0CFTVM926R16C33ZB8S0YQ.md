---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9980
title: '[Low] backendDispatch strips windowLabel but scope chain does not always carry window identity'
---
**File**: `kanban-app/ui/src/lib/command-scope.tsx` lines 220-227\n\n**Issue**: `backendDispatch()` explicitly strips `windowLabel` from params with the comment \"scope chain is the sole mechanism for window identity now.\" However, the backend `dispatch_command` signature no longer accepts `window_label` at all (it was removed from the Tauri command). The stripping is harmless but the comment is misleading — window identity is actually determined by `board_path` in the current implementation, not the scope chain.\n\n**Severity**: Low (clarity)\n**Layer**: Naming/Clarity\n\n**Fix**: Remove the dead `windowLabel` stripping code and update the comment to reflect the actual mechanism (board_path parameter)."