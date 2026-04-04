---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
position_swimlane: container-refactor
title: Extract RustEngineContainer from App.tsx
---
## What

Extract a `RustEngineContainer` component from App.tsx that consolidates all Rust backend bridge providers into a single container with its own `CommandScopeProvider`. This is the first container below `WindowContainer` and provides the foundation that all other containers depend on.

**Files to create/modify:**
- `kanban-app/ui/src/components/rust-engine-container.tsx` (NEW) — wraps `SchemaProvider`, `EntityStoreProvider`, `EntityFocusProvider`, `FieldUpdateProvider`, `UIStateProvider`, `AppModeProvider`, `UndoProvider` into one coherent container
- `kanban-app/ui/src/App.tsx` — replace the nested provider soup with `<RustEngineContainer>`

**Current state:** App.tsx lines 554-658 have 7+ nested providers that are all "Rust engine" concerns:
```
SchemaProvider > EntityStoreProvider > EntityFocusProvider > FieldUpdateProvider > UIStateProvider > AppModeProvider > UndoProvider
```

**Target:** Single `<RustEngineContainer entities={entityStore}>` that wraps children in all these providers plus a `CommandScopeProvider moniker="engine"`.

**Pattern:** Follow container-wrapping convention — one component per file, owns its CommandScopeProvider, wraps children only. No presentation logic.

## Acceptance Criteria
- [ ] `RustEngineContainer` exists as a standalone component file
- [ ] App.tsx provider nesting reduced from 7+ levels to 1 (`<RustEngineContainer>`)
- [ ] All contexts previously available inside the provider soup are still available to descendants
- [ ] `CommandScopeProvider` with moniker `engine` is the outermost wrapper inside the container
- [ ] App still renders correctly (no runtime errors)

## Tests
- [ ] Existing tests in `kanban-app/ui/src/components/app-shell.test.tsx` still pass
- [ ] Existing tests in `kanban-app/ui/src/components/board-view.test.tsx` still pass
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass