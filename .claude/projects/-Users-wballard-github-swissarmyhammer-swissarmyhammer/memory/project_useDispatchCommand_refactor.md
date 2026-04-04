---
name: useDispatchCommand refactor in progress
description: Parallel work converting direct backendDispatch calls to useDispatchCommand across all presenters
type: project
---

A refactor is underway (in another shell/branch) to replace all direct `backendDispatch()` calls in React components with `useDispatchCommand`. This is happening in parallel with the container architecture refactor.

**Why:** Direct `backendDispatch` bypasses the command scope chain. `useDispatchCommand` resolves commands through the scope hierarchy, which is critical now that containers own their own `CommandScopeProvider` scopes.

**How to apply:** When implementing container refactor cards, assume `useDispatchCommand` will be the only dispatch mechanism. Do not introduce new `backendDispatch` calls. If existing code uses `backendDispatch`, note it but don't block on converting it — the parallel refactor will handle it.
