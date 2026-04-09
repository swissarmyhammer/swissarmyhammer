---
assignees:
- claude-code
position_column: todo
position_ordinal: c480
title: '[warning] WindowContainer: backendDispatch called at module scope violates no-module-level-dispatch'
---
kanban-app/ui/src/components/window-container.tsx:177, 226, 261-265, 343-347, 368-375

WindowContainer imports `backendDispatch` from `@/lib/command-scope` and calls it directly from callbacks and effects. Per the project convention `no-module-level-dispatch`, every dispatch should flow through the owning component's hook (useDispatchCommand). Direct backendDispatch calls bypass the scope chain that useDispatchCommand automatically builds.

Several call sites do manually construct scopeChain arrays (`windowScopeChain`), but this is duplicating what useDispatchCommand already provides. In particular, the board-changed listener (line 343) and handleSwitchBoard (line 368) both manually build scope chains.

Suggestion: Replace `backendDispatch` calls with `useDispatchCommand('file.switchBoard')` and pass args through it. The hook automatically attaches the correct scope chain from the React tree. #review-finding