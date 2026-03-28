---
assignees:
- claude-code
position_column: todo
position_ordinal: '8980'
title: App.tsx inspector backdrop onClick calls closeAll() directly instead of dispatching ui.inspector.close_all command
---
**File:** `kanban-app/ui/src/App.tsx`, line 573\n\n**Anti-pattern:** The semi-transparent backdrop behind inspector panels has `onClick={closeAll}`, which calls `invoke(\"dispatch_command\", { cmd: \"ui.inspector.close_all\" })` directly from a local callback. While this does ultimately dispatch to Rust, it bypasses the React command scope chain.\n\n**Current code:**\n```tsx\n<div className={`fixed inset-0 z-20 bg-black/20 ...`}\n  onClick={closeAll}\n/>\n```\n\n**Correct pattern:** Should dispatch `ui.inspector.close_all` through the command scope chain so it is visible to keybinding resolution, command logging, and palette history.\n\n**Severity:** Low. The backdrop is a UI chrome element, not an entity action. The command does reach Rust. This is a consistency issue rather than a functional bug.\n\n**Scope chain impact:** The backdrop close is not visible as a command in the JS-side scope chain -- only in the Rust dispatch log. #review-finding