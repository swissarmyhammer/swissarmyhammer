---
assignees:
- claude-code
position_column: todo
position_ordinal: 8f80
title: Create useDispatchCommand hook — unified command dispatch with automatic scope chain
---
## What

Add `useDispatchCommand` hook to `kanban-app/ui/src/lib/command-scope.tsx`. This is the single way to dispatch commands from React components. Scope chain and boardPath are automatic from context.

**API (overloaded):**
```ts
interface DispatchOptions {
  args?: Record&lt;string, unknown&gt;;
  target?: string;
}

// Ad-hoc: dispatch any command
function useDispatchCommand(): (cmd: string, opts?: DispatchOptions) =&gt; Promise&lt;unknown&gt;;

// Pre-bound: one function per command
function useDispatchCommand(cmd: string): (opts?: DispatchOptions) =&gt; Promise&lt;unknown&gt;;
```

**Internal behavior:**
1. Read `CommandScopeContext`, `ActiveBoardPathContext` from React context (via refs for stability)
2. Compute scope chain from `scopeChainFromScope(scope)`
3. On dispatch: resolve command through scope chain — if it has a frontend `execute` handler, call it. Otherwise dispatch to Rust backend via `invoke("dispatch_command", ...)`
4. Always include `scopeChain` and `boardPath` automatically

**Also in this card:**
- Rename `backendDispatch` to `_backendDispatch` (private, non-exported)
- Re-export `backendDispatch` temporarily as deprecated alias (remove in cleanup card)
- Keep `dispatchCommand` temporarily as deprecated (remove in cleanup card)
- Keep `useExecuteCommand` temporarily as deprecated (remove in cleanup card)

**Files to modify:**
- `kanban-app/ui/src/lib/command-scope.tsx` — add hook, rename internal function
- `kanban-app/ui/src/lib/command-scope.test.tsx` — add tests for the new hook

## Acceptance Criteria
- [ ] `useDispatchCommand()` returns ad-hoc dispatch function
- [ ] `useDispatchCommand("cmd.id")` returns pre-bound dispatch function
- [ ] Both auto-include scope chain from context
- [ ] Both auto-include boardPath from context
- [ ] Frontend `execute` handlers still fire for resolved commands
- [ ] Backend dispatch works for commands without frontend handlers
- [ ] `backendDispatch` still exported (deprecated) for incremental migration

## Tests
- [ ] Ad-hoc dispatch calls invoke with correct scope chain and boardPath
- [ ] Pre-bound dispatch calls invoke with correct cmd and args
- [ ] Frontend execute handler is called when command resolves in scope chain
- [ ] Backend fallback when command not in scope chain
- [ ] Run `cd kanban-app/ui && pnpm test` — all unit tests pass