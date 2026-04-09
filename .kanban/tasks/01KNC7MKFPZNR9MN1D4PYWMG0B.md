---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8180
position_swimlane: container-refactor
title: Extract RustEngineContainer from App.tsx
---
## What

Extract a `RustEngineContainer` component from App.tsx that consolidates all Rust backend bridge providers AND owns entity state management. This is the container that "provides entities, views, and perspectives" — it is fully self-contained, not just a provider wrapper.

**Files to create/modify:**
- `kanban-app/ui/src/components/rust-engine-container.tsx` (NEW) — wraps `SchemaProvider`, `EntityStoreProvider`, `EntityFocusProvider`, `FieldUpdateProvider`, `UIStateProvider`, `AppModeProvider`, `UndoProvider` into one coherent container. **Also owns `entitiesByType` state and all entity Tauri event listeners** (entity-created, entity-removed, entity-field-changed).
- `kanban-app/ui/src/components/rust-engine-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/App.tsx` — replace the nested provider soup with `<RustEngineContainer>`

**Current state:** App.tsx lines 554-658 have 7+ nested providers. Entity state (`entitiesByType`) and the 200+ lines of Tauri event listeners (lines 284-525) that surgically patch entity state are in App.tsx. `EntityStoreProvider` receives entities as a prop from above.

**Target:** `RustEngineContainer` owns:
1. `CommandScopeProvider moniker="engine"`
2. All 7 providers (Schema, EntityStore, EntityFocus, FieldUpdate, UIState, AppMode, Undo)
3. `entitiesByType` state — managed internally, not passed as a prop
4. All entity Tauri event listeners (entity-created, entity-removed, entity-field-changed) — these are engine concerns, not window concerns
5. A `refreshEntities(boardPath)` function exposed via context for WindowContainer to call on board switch
6. Wraps children

This means WindowContainer becomes simpler — it owns board path, open boards list, board-opened/board-changed events, and AppShell. It does NOT own entity state or entity events.

**Pattern:** Follow container-wrapping convention — one component per file, owns its CommandScopeProvider, wraps children only.

## TDD Process
1. Write `rust-engine-container.test.tsx` FIRST with failing tests
2. Tests mock Tauri `invoke`/`listen` APIs
3. Tests verify: providers are present (useSchema, useEntityStore, etc. don't throw), entity event listeners patch state correctly, refreshEntities fetches and updates state
4. Implement until tests pass
5. Refactor

## Acceptance Criteria
- [ ] `RustEngineContainer` exists as a standalone component file
- [ ] `rust-engine-container.test.tsx` exists with tests written before implementation
- [ ] App.tsx provider nesting reduced from 7+ levels to 1 (`<RustEngineContainer>`)
- [ ] `entitiesByType` state and entity event listeners live inside RustEngineContainer, not App/WindowContainer
- [ ] All contexts previously available inside the provider soup are still available to descendants
- [ ] `CommandScopeProvider` with moniker `engine` is the outermost wrapper inside the container
- [ ] App still renders correctly (no runtime errors)

## Tests
- [ ] `rust-engine-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] Existing tests in `kanban-app/ui/src/components/app-shell.test.tsx` still pass
- [ ] Existing tests in `kanban-app/ui/src/components/board-view.test.tsx` still pass
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass