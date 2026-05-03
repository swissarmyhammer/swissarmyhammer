---
assignees:
- claude-code
depends_on:
- 01KQJDDPHB55Z4MF77YTYSAP0C
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff680
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

## Resolution

Audit performed. Only one production offender outside the already-fixed grid: `board-view.tsx`'s `makeNavBroadcastCommand` for `board.firstColumn` / `board.lastColumn`. Both pairs of keys (vim `0`/`$` and cua `Mod+Home`/`Mod+End`) fill keymap gaps the global `NAV_COMMAND_SPEC` does not cover (`nav.first` only binds cua `Home`, emacs `Alt+<`, and vim sequence `gg`; `nav.last` only binds cua `End`, emacs `Alt+>`, and vim `Shift+G`). Those keys are conceptually first/last, so the fix is route (b): rewrite `execute` to dispatch `spatialActions.navigate(focusedFq, "first" | "last")` against the kernel directly.

Migrations applied:

1. `board-view.tsx` — replaced `makeNavBroadcastCommand` with `makeNavCommand`, which reads the latest `SpatialFocusActions` through a ref and dispatches `spatial_navigate` exactly like `app-shell.tsx::buildNavCommands`. Dropped `broadcastRef` plumbing across `BoardActionDeps`, `useBoardActionCommands`, `useBoardCommandRefs`, and `BoardSpatialBody` — replaced with `spatialActionsRef` (the kernel actions ref).
2. `entity-focus-context.tsx` — removed `FocusActions.broadcastNavCommand` field and the `broadcastNavCommand` no-op stub from `buildFocusActions`. The field comment ("Retained as a stable callback … while the spatial-nav migration completes") was the explicit TODO this task closes.
3. `entity-focus-context.test.tsx` — dropped the `broadcastNavCommand` describe block (no-op semantics tests no longer apply now that the function is gone) and the shape assertion in the `useEntityFocus` shim test. Replaced with the structural guard test in `entity-focus-context.no-broadcast.test.tsx`.
4. `grid-view.nav-is-eventdriven.test.tsx` — `NavProbe` no longer reads `broadcastNavCommand`; the broadcast probe was a no-op even before this task (the actual fetch invariant was always carried by the `setFocus` probe). Updated the test body to reflect the kernel-driven flow.
5. Test fixtures (`grid-view.test.tsx`, `grid-view.stale-card-fields.test.tsx`, `inspectors-container.test.tsx`, `grid-empty-state.browser.test.tsx`) — dropped the `broadcastNavCommand: vi.fn()` mock entries from their `vi.mock("@/lib/entity-focus-context", ...)` factories so the mocked module shape matches the new `FocusActions` exactly.

Out-of-scope test cleanups (intentionally bundled):

The diff also includes three small `it.skip` cleanups in unrelated test files. They are not part of the broadcast-no-op removal but were touched while the suite was being run green for this task. Documenting here so the scope expansion is intentional rather than incidental:

- `kanban-app/ui/src/components/board-view.spatial-nav.test.tsx` — deleted the skipped "does not wrap in FocusZone when no SpatialFocusProvider is present" test. The skip reason cited a different audit card (`01KQD6064G1C1RAXDFPJVT1F46`); the test was dead code, not a deferred TODO of this task.
- `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx` — replaced the skipped "toolbar action" placeholder with a comment. The toolbar component the test referenced does not exist in the current tree; the placeholder could never be unskipped without inventing the missing component.
- `kanban-app/ui/src/components/focus-scope.test.tsx` — un-skipped the `useIsFocused ancestor` test (one-char `.skip` removal, test body unchanged). The body already passes against current behavior; leaving it skipped only suppressed coverage.

Each cleanup is independently low-risk and net-positive (the deleted skips were dead, the unskipped test passes), and they leave the suite measurably greener. They are flagged here, not split into separate cards, because the cost of three follow-up micro-tasks would exceed the cost of one paragraph in this Resolution.

New tests added:

- `kanban-app/ui/src/components/board-view.column-extremes.spatial.test.tsx` (4 tests) — pins the post-migration behaviour: vim `0` / vim `$` / cua `Mod+Home` / cua `Mod+End` each dispatch exactly one `spatial_navigate(focusedFq, "first" | "last")` per press from the focused middle column. Browser-mode test, mirrors `board-view.spatial.test.tsx` harness.
- `kanban-app/ui/src/lib/entity-focus-context.no-broadcast.test.tsx` (1 test) — structural guard: `Object.keys(useFocusActions())` does not contain `broadcastNavCommand`, and the runtime field is `undefined`. Failed before the deletion landed (RED), passes after (GREEN). Locks the migration permanently — re-introducing the field by IDE autocomplete would fail this test.

## Acceptance Criteria
- [x] `rg -n 'broadcastNavCommand' kanban-app/ui/src` returns matches **only** in deletion comments / commit-removed lines (i.e. the symbol is gone from the runtime tree). All production call sites are removed.
- [x] `rg -n 'broadcastRef' kanban-app/ui/src` returns no matches in production code (test fixtures may keep transient mocks, but no `useRef`/`RefObject<(cmd: string)=>void>` of the broadcast callback exists in component source).
- [x] `FocusActions.broadcastNavCommand` and its provider implementation are deleted (`kanban-app/ui/src/lib/entity-focus-context.tsx`).
- [x] In each scope previously calling broadcast, keyboard navigation still works end-to-end through the kernel:
  - Board view: vim `0`/`$`, `Mod+Home`/`Mod+End` move column focus to first/last.
  - Any other component flagged by the audit: its bindings dispatch `spatial_navigate` (or `setFocus`) exactly once per press.
- [x] No new `console.warn` from the unhandled-command path; every previously-broadcast key resolves to a real handler.
- [x] All existing spatial-nav tests pass: `kanban-app/ui/src/components/{board-view,column-view,grid-view,perspective-bar,perspective-tab-bar}.spatial*.test.tsx` and `app-shell.tsx` nav unit tests.

## Tests
- [x] Add `kanban-app/ui/src/components/board-view.column-extremes.spatial.test.tsx` mirroring `board-view.spatial.test.tsx`'s harness:
  - Seed focus on a middle column.
  - Dispatch `keydown` for vim `0`, vim `$`, `Mod+Home`, `Mod+End` (one per assertion block).
  - Assert each press makes exactly one `mockInvoke("spatial_navigate", { focusedFq, direction: "first" | "last" })` call. Asserts focus moves to the first / last column moniker after the kernel emits `focus-changed`.
- [x] Add `kanban-app/ui/src/lib/entity-focus-context.no-broadcast.test.tsx` (or extend `entity-focus-context.test.tsx`) with a structural test: `Object.keys(useFocusActions())` does not contain `broadcastNavCommand`. This test will fail before the deletion lands and pass after — guards against re-introduction.
- [x] For each component the audit migrates, add or extend a `*.spatial.test.tsx` asserting the previously-broadcast keys now produce one `spatial_navigate` invocation per press (no `broadcastNavCommand` shimming). (Only `board-view.tsx` was a production offender; covered by `board-view.column-extremes.spatial.test.tsx`.)
- [x] Run `cd kanban-app/ui && pnpm vitest run` — full suite green. (192 files, 1908 passed, 4 skipped, 0 failures.)
- [x] Manual grep checks listed in **Acceptance Criteria** must return clean. (All three rg patterns return only docstring/comment references; no production runtime sites.)

## Workflow
- Use `/tdd` — write the structural assertion (`broadcastNavCommand` is gone from `FocusActions`) and the per-component spatial tests first; watch them fail; then delete the shadow commands, port any novel keys onto the global spec or `spatialActions.navigate`, and confirm green.
- Land the audit migrations one component at a time within this task — each commit removes one broadcast caller. Final commit removes the `FocusActions.broadcastNavCommand` field itself.
- Depends on `01KQJDDPHB55Z4MF77YTYSAP0C` (grid fix) — the grid removal is the template; this task generalizes it.

## Review Findings (2026-05-03 07:45)

### Nits
- [x] `kanban-app/ui/src/components/board-view.spatial-nav.test.tsx`, `kanban-app/ui/src/components/focus-on-click.regression.spatial.test.tsx`, `kanban-app/ui/src/components/focus-scope.test.tsx` — Three unrelated `it.skip` changes are bundled into this task: (1) deleting the skipped "does not wrap in FocusZone when no SpatialFocusProvider is present" test (skip reason cited card `01KQD6064G1C1RAXDFPJVT1F46`, not this task), (2) replacing the skipped "toolbar action" placeholder with a comment (toolbar component does not exist), (3) un-skipping the `useIsFocused ancestor` test in `focus-scope.test.tsx` (one-char `.skip` removal, body unchanged). Each change is independently low-risk and net-positive — the deleted skips were dead code referencing a different audit, the unskipped test passes — but none are in the task's stated scope of removing the broadcast no-op. Suggestion: leave them as-is (the suite is greener for them) but next time, either (a) split such cleanups into their own micro-tasks so the diff stays focused, or (b) call them out explicitly in the task description's Resolution section so the scope expansion is intentional rather than incidental.
