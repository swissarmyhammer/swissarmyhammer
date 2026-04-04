---
assignees:
- claude-code
depends_on:
- 01KNC7MKFPZNR9MN1D4PYWMG0B
position_column: done
position_ordinal: fffffffffffffffffff480
position_swimlane: container-refactor
title: Extract StoreContainer — binds filesystem path to entity store
---
## What

Add a `StoreContainer` component between `WindowContainer` and `BoardContainer` in the hierarchy. It owns the binding between a filesystem path (`.kanban` directory) and the board handle / entity store.

**Scope chain after this:**
```
window:main → store:/path/to/.kanban → board:01ABC → column:todo → task:task-1
```

`StoreContainer` is the component that knows \"this subtree operates on this `.kanban` directory.\" It provides a `FocusScope` with `renderContainer={false}` and `moniker=\"store:{canonicalPath}\"`. The backend can extract the store path from the scope chain to resolve the board handle — replacing the explicit `boardPath` IPC parameter.

**Files to create/modify:**
- `kanban-app/ui/src/components/store-container.tsx` (NEW) — `FocusScope(renderContainer=false, moniker=\"store:{path}\")`, provides board handle context to children
- `kanban-app/ui/src/components/store-container.test.tsx` (NEW) — TDD: tests first
- `kanban-app/ui/src/App.tsx` — insert StoreContainer between WindowContainer and BoardContainer

**Backend changes (same card or dependency):**
- `kanban-app/src/commands.rs` — `dispatch_command_internal`: resolve board handle from `store:` moniker in scope chain via `resolve_entity_id(\"store\")` → canonicalize → lookup in `AppState::boards`. Remove `board_path` parameter from `dispatch_command` signature.
- `swissarmyhammer-commands/src/context.rs` — add `resolve_store_path()` helper that extracts and returns the path from a `store:` moniker in scope chain

**Target hierarchy:**
```
WindowContainer (window:main)
  └─ RustEngineContainer (engine)
       └─ StoreContainer (store:/path/to/.kanban)  ← NEW
            └─ BoardContainer (board:01ABC)
                 └─ ...
```

## Acceptance Criteria
- [ ] `StoreContainer` exists, provides `store:{path}` moniker in scope chain
- [ ] `store-container.test.tsx` exists with tests written before implementation
- [ ] Backend resolves board handle from `store:` moniker in scope chain
- [ ] `boardPath` parameter removed from `dispatch_command` Tauri command
- [ ] All commands still resolve the correct board

## Tests
- [ ] `store-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] New Rust test: scope chain with `store:/path` resolves correct board handle
- [ ] `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] `cd kanban-app/ui && pnpm vitest run` — all pass
- [ ] Manual: open board, run commands, verify they target the correct board