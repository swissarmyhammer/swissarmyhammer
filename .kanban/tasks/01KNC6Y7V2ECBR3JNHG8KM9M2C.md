---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffd580
title: Fix InspectButton dispatching ui.inspect to board instead of task entity
---
## What

The (i) inspect button on entity cards opens the inspector for the **board** instead of the clicked task. Root cause is a scope-chain escalation bug in the dispatch chain.

### Root Cause

`InspectButton` (`kanban-app/ui/src/components/entity-card.tsx:158-176`) uses `resolveCommand(scope, "ui.inspect")` → `dispatchCommand(cmd, ...)`. The resolved command has an `execute` handler from `useEntityCommands` (line 86). That handler calls `dispatch("ui.inspect", { target: entityMoniker })` where `dispatch` comes from `useDispatchCommand()` called at line 126 of `entity-commands.ts` — which captures scope from the **parent** of the entity card's `FocusScope` (because `useEntityCommands` is called at line 86, before the `FocusScope` starts at line 97).

The parent-scope `dispatch` then resolves `ui.inspect` again, this time finding the **board-level** command from `useEntityCommands("board", "board")` at `board-view.tsx:79`. That command's `execute` handler dispatches with `target: "board:board"` — inspecting the board.

The `opts.target` containing the task moniker is silently discarded at `command-scope.tsx:378` because `useDispatchCommand` calls `resolved.execute()` without forwarding `opts`.

### Fix

In `entity-card.tsx`, replace the `resolveCommand` → `dispatchCommand` pattern in `InspectButton` with a direct `backendDispatch` call. The scope chain from `CommandScopeContext` already includes the entity moniker (e.g. `["task:abc", "column:todo", "board:board", "window:main"]`), so the backend's `first_inspectable()` (`ui_commands.rs:17-25`) will find the task as the first inspectable entry — no explicit target needed.

```tsx
function InspectButton() {
  const scope = useContext(CommandScopeContext);
  const boardPath = useActiveBoardPath();
  const chain = useMemo(() => scopeChainFromScope(scope), [scope]);
  return (
    <button
      type=\"button\"
      className=\"...\"
      onClick={(e) => {
        e.stopPropagation();
        backendDispatch({
          cmd: \"ui.inspect\",
          scopeChain: chain,
          ...(boardPath ? { boardPath } : {}),
        }).catch(console.error);
      }}
      title=\"Inspect\"
    >
      <Info className=\"h-3.5 w-3.5\" />
    </button>
  );
}
```

### Files to modify
- `kanban-app/ui/src/components/entity-card.tsx` — `InspectButton` function (lines 158-176): replace `resolveCommand`/`dispatchCommand` with `backendDispatch`
- Remove unused imports: `resolveCommand`, `dispatchCommand` (if no other usages remain in this file)

## Acceptance Criteria
- [ ] Clicking (i) on a task card inspects that task, not the board
- [ ] Clicking (i) on a tag card inspects that tag
- [ ] Inspector shows correct entity fields matching the clicked card
- [ ] Scope chain sent to backend includes the entity moniker as first entry

## Tests
- [ ] Update `kanban-app/ui/src/components/entity-card.test.tsx` — assert that clicking the inspect button calls `backendDispatch` (or `invoke(\"dispatch_command\", ...)`) with `cmd: \"ui.inspect\"` and a scope chain whose first entry matches the entity moniker
- [ ] `cd kanban-app/ui && pnpm test` — all pass