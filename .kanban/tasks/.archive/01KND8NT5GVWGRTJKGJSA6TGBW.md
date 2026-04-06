---
assignees:
- claude-code
position_column: todo
position_ordinal: c380
title: '[warning] WindowContainer: duplicate ActiveBoardPath context providers'
---
kanban-app/ui/src/components/window-container.tsx:386-388

WindowContainer renders BOTH `ActiveBoardPathProvider` (line 386, from command-scope.ts) AND its own `ActiveBoardPathContext.Provider` (line 388). The first is from `@/lib/command-scope` and the second is its own context. Components importing `useActiveBoardPath` from window-container.tsx get the inner one, but components using `useActiveBoardPath` from command-scope.ts get the outer one.

This dual-provider pattern is confusing and fragile. If the two values ever diverge, components will get inconsistent board paths depending on which import they use.

Suggestion: Remove one of the two providers. If `ActiveBoardPathProvider` from command-scope is still needed by legacy consumers, re-export the hook from one canonical location and remove the duplicate context. #review-finding