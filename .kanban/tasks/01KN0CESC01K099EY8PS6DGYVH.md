---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8180
title: '[Low] dispatch_command_internal uses Box::pin recursive call for dynamic commands'
---
**File**: `kanban-app/src/commands.rs` — `dispatch_command_internal` dynamic prefix handling\n\n**Issue**: The `view.switch:*`, `board.switch:*`, and `window.focus:*` prefix interception uses `Box::pin(dispatch_command_internal(...)).await` for recursive dispatch. This is functionally correct but:\n1. Each recursive call allocates a new boxed future on the heap\n2. The `window.focus:*` path returns early without going through the standard result-processing pipeline (no UIState emit, no menu update)\n3. There is no depth limit — a malicious or buggy command like `board.switch:board.switch:...` would recurse\n\n**Severity**: Low (design)\n**Layer**: Design/Architecture\n\n**Fix**: Consider a simple loop with an enum for the rewrite target instead of recursion. Or add a depth counter parameter (max 1 rewrite). The `window.focus:` early return is likely intentional but should be documented."