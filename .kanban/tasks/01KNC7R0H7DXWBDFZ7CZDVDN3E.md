---
assignees:
- claude-code
depends_on:
- 01KNC7QESP1X7G2SCPNXK6R64F
- 01KNC7PWDAJEKEFECDX90Q1WJ2
- 01KNCRHRDYZBSYHKT2436G8RGX
position_column: done
position_ordinal: ffffffffffffffffffffffffef80
title: Slim App.tsx to pure container composition
---
## What

Final cleanup: reduce App.tsx to a pure composition of containers with no logic, no state, no event handlers. This is the capstone card after all containers are extracted.

**Files to modify:**
- `kanban-app/ui/src/App.tsx` — should become ~50 lines: imports + container tree + QuickCaptureApp

**Target App.tsx:**
```tsx
function App() {
  return (
    <WindowContainer>
      <AppModeContainer>
        <RustEngineContainer>
          <BoardContainer>
            <div className="h-screen bg-background text-foreground flex flex-col">
              <NavBar />
              <ViewsContainer>
                <ViewContainer>
                  <PerspectivesContainer>
                    <PerspectiveContainer>
                      {/* BoardView or GridView rendered by ViewContainer */}
                    </PerspectiveContainer>
                  </PerspectivesContainer>
                </ViewContainer>
              </ViewsContainer>
              <ModeIndicator />
            </div>
            <InspectorContainer />
          </BoardContainer>
        </RustEngineContainer>
      </AppModeContainer>
    </WindowContainer>
  );
}
```

**QuickCaptureApp** should also be simplified to use `<RustEngineContainer>` instead of duplicating the provider tree.

**What to verify:**
- No `useState`, `useEffect`, `useCallback`, `useMemo` remain in App.tsx
- No Tauri `invoke`, `listen`, or `emit` imports
- No inline component definitions (InspectorSyncBridge, ViewCommandScope, ActiveViewRenderer, InspectorPanel all gone)
- Only container imports + composition

## TDD Process
1. Write/update `App.test.tsx` FIRST — test that App renders the container tree in correct order, no state hooks present
2. Implement until tests pass
3. Refactor

## Acceptance Criteria
- [ ] App.tsx is under 80 lines total (including QuickCaptureApp)
- [ ] No state management in App.tsx
- [ ] No event listeners in App.tsx
- [ ] Container tree is clearly readable as a hierarchy
- [ ] AppModeContainer wraps immediately inside WindowContainer
- [ ] QuickCaptureApp uses RustEngineContainer instead of duplicating providers
- [ ] All functionality preserved — app works identically to before

## Tests
- [ ] App-level test verifies container tree composition (written first, RED → GREEN)
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Run `cd kanban-app && pnpm tsc --noEmit` — no type errors
- [ ] Manual: full smoke test (open board, switch views, switch perspectives, inspect entities, drag tasks, undo/redo)