---
assignees:
- claude-code
position_column: todo
position_ordinal: c980
title: '[nit] ViewsContainer: backendDispatch in execute callback instead of useDispatchCommand'
---
kanban-app/ui/src/components/views-container.tsx:47-52

ViewsCommandScope builds command definitions with inline `execute` callbacks that call `backendDispatch` directly:

```tsx
execute: () => {
  backendDispatch({
    cmd: `view.switch:${view.id}`,
    scopeChain: [`window:${WINDOW_LABEL}`],
  }).catch(console.error);
},
```

This manually constructs the scope chain with only the window label, missing any intermediate scopes (mode, board, etc.). Per the `no-module-level-dispatch` convention, dispatch should go through the React scope chain.

However, since these commands are registered in the CommandScopeProvider and executed by the framework, the scope chain is rebuilt at dispatch time. This is a minor inconsistency rather than a bug -- the scope chain in the command definition is only used if the command is invoked directly, not through the palette.

Suggestion: Consider using `useDispatchCommand` if the API supports registering commands with dispatch-time scope resolution. #review-finding