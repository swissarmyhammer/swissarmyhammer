---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: BoardSelector tear-off button calls invoke(\"create_window\") directly, bypassing command dispatch
---
**File:** `kanban-app/ui/src/components/board-selector.tsx`, lines 120-133\n\n**Anti-pattern:** The tear-off button calls `invoke(\"create_window\", { boardPath: selectedPath })` directly instead of dispatching through the command system. There is already a `window.new` command registered in AppShell (app-shell.tsx line 320-327), but it does not accept a boardPath argument.\n\n**Current code:**\n```tsx\n<Button onClick={() => {\n  invoke(\"create_window\", { boardPath: selectedPath }).catch(console.error);\n}}>\n```\n\n**Correct pattern:** Should dispatch a command like `window.new` with args `{ boardPath: selectedPath }` through the scope chain, or use `invoke(\"dispatch_command\", { cmd: \"window.new\", args: { boardPath } })`.\n\n**Severity:** Warning. The `window.new` command in AppShell does not currently accept a board path parameter, so this is also a gap in the command definition -- but the direct `invoke` bypass is the anti-pattern.\n\n**Scope chain impact:** Not logged, not visible in command palette history, not interceptable by middleware. #review-finding