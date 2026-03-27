---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Add claimWhen primitive to FocusScope
---
## What

Add two primitives to FocusScope:

### 1. `FocusScopeContext` + `useParentFocusScope()`

FocusScope provides a `FocusScopeContext` with its moniker. Any child component calls `useParentFocusScope()` to get the nearest ancestor FocusScope's moniker. This is how a pill knows its parent field moniker without prop threading.

```tsx
const FocusScopeContext = createContext<string | null>(null);

// Inside FocusScope render:
<FocusScopeContext.Provider value={moniker}>
  ...
</FocusScopeContext.Provider>

// Hook for children:
export function useParentFocusScope(): string | null {
  return useContext(FocusScopeContext);
}
```

CommandScopeProviders (which aren't FocusScopes) don't set this context, so it naturally skips over them to the nearest FocusScope ancestor.

### 2. `claimWhen` prop + claim broadcast

`claimWhen` is an array of `{ command: string, when: (focusedMoniker: string | null) => boolean }`. Each FocusScope registers its predicates with EntityFocusProvider on mount. When a navigation command fires, EntityFocusProvider broadcasts it — iterating registered predicates, **stopping at the first match** (short-circuit), and calling `setFocus(claimantMoniker)`.

### Files to modify

- **`kanban-app/ui/src/components/focus-scope.tsx`** — add `FocusScopeContext`, `useParentFocusScope()`, `claimWhen` prop. FocusScope wraps children in `FocusScopeContext.Provider`. On mount, registers claimWhen predicates; on unmount, unregisters.
- **`kanban-app/ui/src/lib/entity-focus-context.tsx`** — add claim predicate registry: `registerClaimPredicates(moniker, predicates)` / `unregisterClaimPredicates(moniker)` and `broadcastNavCommand(commandId)` which evaluates predicates with current focusedMoniker, first match wins.

### Registration order / short-circuit

React effects fire children-first (depth-first). Child FocusScopes register before parent FocusScopes. More-specific scopes (pills) are checked before less-specific (field rows). First match wins, loop stops.

### API

```tsx
// A pill uses useParentFocusScope to know its field moniker
const parentMoniker = useParentFocusScope();

<FocusScope
  moniker={pillMoniker}
  commands={entityCommands}
  claimWhen={[
    { command: \"nav.right\", when: (f) => f === parentMoniker },
    { command: \"nav.right\", when: (f) => f === prevSiblingMoniker },
  ]}
/>
```

## Acceptance Criteria

- [ ] `useParentFocusScope()` returns nearest ancestor FocusScope's moniker
- [ ] `useParentFocusScope()` returns null when no ancestor FocusScope exists
- [ ] `useParentFocusScope()` skips CommandScopeProviders that aren't FocusScopes
- [ ] FocusScope accepts `claimWhen` prop
- [ ] Predicates are registered/unregistered with EntityFocusProvider on mount/unmount
- [ ] `broadcastNavCommand(commandId)` evaluates predicates with current focusedMoniker
- [ ] First matching predicate claims focus, evaluation stops (short-circuit)
- [ ] Unmounted FocusScopes don't leave stale predicates

## Tests

- [ ] `focus-scope.test.tsx` — `useParentFocusScope()` returns parent FocusScope moniker
- [ ] `focus-scope.test.tsx` — `useParentFocusScope()` skips CommandScopeProvider, returns grandparent FocusScope moniker
- [ ] `focus-scope.test.tsx` — `useParentFocusScope()` returns null at root
- [ ] `entity-focus-context.test.tsx` — broadcast with one matching predicate claims focus
- [ ] `entity-focus-context.test.tsx` — broadcast with no match leaves focus unchanged
- [ ] `entity-focus-context.test.tsx` — first match wins (short-circuit)
- [ ] `entity-focus-context.test.tsx` — unmounted scope's predicate is not evaluated
- [ ] `pnpm vitest run` passes"