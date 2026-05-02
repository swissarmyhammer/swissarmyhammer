---
assignees:
- claude-code
depends_on:
- 01KQJDDPHB55Z4MF77YTYSAP0C
position_column: todo
position_ordinal: ad80
project: spatial-nav
title: 'Audit: remove duplicate scope-local nav commands that shadow global nav.* and route through no-op broadcast'
---
## What

Sister task to `01KQJDDPHB55Z4MF77YTYSAP0C` (grid keyboard nav fix). The grid is not the only component that registers scope-local nav commands which shadow the global `nav.*` set and execute via the no-op `broadcastNavCommand` (`kanban-app/ui/src/lib/entity-focus-context.tsx` lines 175–183).

The single source of truth for spatial navigation is `NAV_COMMAND_SPEC` / `buildNavCommands` in `kanban-app/ui/src/components/app-shell.tsx` (lines 226–301). Each entry dispatches `spatial_navigate(focusedFq, direction)` to the Rust kernel. Any other CommandDef that:

  1. Binds a vim/cua/emacs key already covered by `NAV_COMMAND_SPEC`, AND
  2. Executes via `broadcastRef.current(...)` / `broadcastNavCommand(...)` instead of `spatialActions.navigate(...)` or `setFocus(...)`

…is dead code that silently breaks keyboard navigation in its scope.

**Known offender (preliminary scan)** — confirm and fix as part of this task:

- `kanban-app/ui/src/components/board-view.tsx` — `useBoardCommandRefs` (line ~992) reads `broadcastNavCommand` from `useFocusActions()` and threads a `broadcastRef` into `useBoardActionCommands` (line ~710). `makeNavBroadcastCommand` (lines 683–698) builds:
  - `board.firstColumn` keys `{ vim: "0", cua: "Mod+Home" }` → broadcasts `nav.first`
  - `board.lastColumn`  keys `{ vim: "$", cua: "Mod+End"  }` → broadcasts `nav.last`

  Both broadcasts hit the no-op. The global `nav.first` / `nav.last` commands in `app-shell.tsx` already own these keys (`nav.first`: `cua: "Home"`, `emacs: "Alt+<"`, vim sequence `gg`; `nav.last`: `vim: "Shift+G"`, `cua: "End"`, `emacs: "Alt+>"`). Mod+Home / Mod+End and vim `0`/`$` are not in the global spec — for those, either move them into `NAV_COMMAND_SPEC` or rewrite the command's `execute` to call `spatialActions.navigate(focusedFq, "first" | "last")` directly. **No new no-op routing.**

**Audit scope**: search the whole `kanban-app/ui/src/` tree for the pattern. Three orthogonal greps cover it:

```
rg -n 'broadcastNavCommand'         kanban-app/ui/src   # all consumers of the no-op
rg -n 'broadcastRef\.current\('     kanban-app/ui/src   # all forwards into the broadcast no-op
rg -n 'execute:.*broadcast'         kanban-app/ui/src   # CommandDef.execute closures that broadcast
```

For each match outside `entity-focus-context.tsx` and the test files, classify the binding:

  - **Direction key already in `NAV_COMMAND_SPEC`** (k/j/h/l, ArrowUp/Down/Left/Right, Home/End, Ctrl+p/n/b/f, Alt+</>, Shift+G, vim sequence `gg`) → delete the local CommandDef. The global wins.
  - **Direction key NOT yet in `NAV_COMMAND_SPEC`** but conceptually one of {up, down, left, right, first, last} → either (a) add the key to the global spec's `keys` map so one command owns it, or (b) rewrite `execute` to call `spatialActions.navigate(focusedFq, direction)` against the kernel directly (pattern: `useOptionalSpatialFocusActions()` ref, mirror `app-shell.tsx::buildNavCommands`).
  - **Truly novel local nav** (e.g. row-extreme, column-extreme that the kernel doesn't yet model) → keep the local command but stop broadcasting; call `setFocus(composeFq(zoneFq, asSegment(targetMoniker)))` directly.

Once every grid/board/etc. caller stops invoking `broadcastNavCommand`, delete the field from `FocusActions` (`entity-focus-context.tsx` line 182), the build helper that produces it (search `buildFocusActions`), and any test that asserts on it. The interface comment ("Retained as a stable callback … existing call sites … compile without churn while the spatial-nav migration completes") is the explicit migration TODO this task closes.

## Acceptance Criteria
- [ ] `rg -n 'broadcastNavCommand' kanban-app/ui/src` returns matches **only** in deletion comments / commit-removed lines (i.e. the symbol is gone from the runtime tree). All production call sites are removed.
- [ ] `rg -n 'broadcastRef' kanban-app/ui/src` returns no matches in production code (test fixtures may keep transient mocks, but no `useRef`/`RefObject<(cmd: string)=>void>` of the broadcast callback exists in component source).
- [ ] `FocusActions.broadcastNavCommand` and its provider implementation are deleted (`kanban-app/ui/src/lib/entity-focus-context.tsx`).
- [ ] In each scope previously calling broadcast, keyboard navigation still works end-to-end through the kernel:
  - Board view: vim `0`/`$`, `Mod+Home`/`Mod+End` move column focus to first/last.
  - Any other component flagged by the audit: its bindings dispatch `spatial_navigate` (or `setFocus`) exactly once per press.
- [ ] No new `console.warn` from the unhandled-command path; every previously-broadcast key resolves to a real handler.
- [ ] All existing spatial-nav tests pass: `kanban-app/ui/src/components/{board-view,column-view,grid-view,perspective-bar,perspective-tab-bar}.spatial*.test.tsx` and `app-shell.tsx` nav unit tests.

## Tests
- [ ] Add `kanban-app/ui/src/components/board-view.column-extremes.spatial.test.tsx` mirroring `board-view.spatial.test.tsx`'s harness:
  - Seed focus on a middle column.
  - Dispatch `keydown` for vim `0`, vim `$`, `Mod+Home`, `Mod+End` (one per assertion block).
  - Assert each press makes exactly one `mockInvoke("spatial_navigate", { focusedFq, direction: "first" | "last" })` call. Asserts focus moves to the first / last column moniker after the kernel emits `focus-changed`.
- [ ] Add `kanban-app/ui/src/lib/entity-focus-context.no-broadcast.test.tsx` (or extend `entity-focus-context.test.tsx`) with a structural test: `Object.keys(useFocusActions())` does not contain `broadcastNavCommand`. This test will fail before the deletion lands and pass after — guards against re-introduction.
- [ ] For each component the audit migrates, add or extend a `*.spatial.test.tsx` asserting the previously-broadcast keys now produce one `spatial_navigate` invocation per press (no `broadcastNavCommand` shimming).
- [ ] Run `cd kanban-app/ui && pnpm vitest run` — full suite green.
- [ ] Manual grep checks listed in **Acceptance Criteria** must return clean.

## Workflow
- Use `/tdd` — write the structural assertion (`broadcastNavCommand` is gone from `FocusActions`) and the per-component spatial tests first; watch them fail; then delete the shadow commands, port any novel keys onto the global spec or `spatialActions.navigate`, and confirm green.
- Land the audit migrations one component at a time within this task — each commit removes one broadcast caller. Final commit removes the `FocusActions.broadcastNavCommand` field itself.
- Depends on `01KQJDDPHB55Z4MF77YTYSAP0C` (grid fix) — the grid removal is the template; this task generalizes it.
