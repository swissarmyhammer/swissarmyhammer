---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffed80
project: spatial-nav
title: Revert in-session spatial nav edits back to the last known-working baseline
---
## What

During an ad-hoc debugging session, six files were edited without failing tests first:

- `kanban-app/ui/src/components/data-table.tsx` — cell FocusScope wrapping, RowSelector FocusScope wrapping, row `spatial={false}`
- `kanban-app/ui/src/components/focus-scope.tsx` — new `spatial` prop on FocusScope + `FocusScopeElementRefContext` for `renderContainer={false}` consumers
- `kanban-app/ui/src/components/left-nav.tsx` — FocusScope wrapping on each view button
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — ScopedPerspectiveTab changed from `CommandScopeProvider` to `FocusScope`
- `kanban-app/ui/src/lib/keybindings.ts` — `createKeyHandler` refactored (helper extraction) + diagnostics (diagnostics already removed, refactor kept)
- `kanban-app/ui/src/main.tsx` — temporary `window.__spatialDump` debug handle

The first, correct fix landed: JS invoke now sends camelCase `layerKey` / `parentScope` instead of snake_case, which got rects registering for 80+ board entries. Beyond that, every change was untested and broke more than it fixed — board nav regressed, inspector nav regressed ("totally fucked, sorry"), grid nav became a jumping mess.

### Required state

Back to the point where the user confirmed:
- "nav works on board and inspector, not grid"

That is: rects register correctly (the camelCase fix); grid cell nav is known-broken; inspector and board nav work; no half-baked FocusScope wrappings anywhere else.

### Approach

`git diff HEAD` each of the six files. Keep ONLY the minimal camelCase fix in `focus-scope.tsx` (and the matching `.catch(() => {})` cleanup). Revert everything else to the last committed version. Do not apply any "improvement" without a failing test from one of the sibling tasks.

### Subtasks

- [ ] `data-table.tsx` — revert to committed version. (Losing: cell FocusScope, row selector FocusScope, row `spatial={false}`, `useFocusScopeElementRef` import.)
- [ ] `focus-scope.tsx` — revert everything EXCEPT the camelCase change to the `spatial_register` invoke (`layerKey` / `parentScope`). Remove the `spatial` prop, the `FocusScopeElementRefContext`, and the associated export. Keep `.catch(() => {})` silent as before.
- [ ] `left-nav.tsx` — revert to committed version.
- [ ] `perspective-tab-bar.tsx` — revert to committed version.
- [ ] `keybindings.ts` — keep the `createKeyHandler` helper extraction (it was validator-driven and neutral); revert anything else (I already removed diagnostics, just confirm).
- [ ] `main.tsx` — revert to committed version. The `window.__spatialDump` handle was debug-only.
- [ ] Run `pnpm vitest run` — all existing tests should pass (they do today against the committed baseline).
- [ ] Smoke in app: board click → h/j/k/l works; open inspector → field nav works; grid → known-broken (to be re-fixed in sibling tasks).

### Acceptance

- [ ] `git diff HEAD` shows ONLY the two-word field rename in `focus-scope.tsx:89-90` (and possibly the equivalent `createKeyHandler` refactor).
- [ ] Board and inspector nav are restored to working.
- [ ] Grid nav is allowed to be broken (separate task fixes it with TDD).
- [ ] No new untested FocusScope wrappers, no debug window handles, no diagnostic log statements.

## Review Findings (2026-04-20 12:11)

### Warnings
- [x] `kanban-app/ui/src/components/left-nav.tsx:28-34` — Scope leak: a new 7-line JSDoc block was added above `export function LeftNav()` that is not present on `HEAD`. The task's first acceptance criterion explicitly states `git diff HEAD` should show ONLY the `focus-scope.tsx:89-90` rename and the `createKeyHandler` refactor, and the `left-nav.tsx` subtask says "revert to committed version." The doc text itself is harmless and accurate, but it violates the stated acceptance criteria for a pure-revert task. Either (a) drop the JSDoc from `left-nav.tsx` to satisfy the acceptance gate, or (b) stage it as a separate follow-up commit/task so the revert stays bit-for-bit minimal. Recommend (a). **Resolved: dropped the JSDoc block; `git diff HEAD -- kanban-app/ui/src/components/left-nav.tsx` is now empty.**

### Nits
- [x] `kanban-app/ui/src/components/focus-scope.test.tsx:829,832` — The test rename (`layer_key` → `layerKey`) is correct and necessary (the previous assertion was codifying the original bug), but the acceptance criteria list in the task description does not call out that `focus-scope.test.tsx` is also in scope. Consider amending the task's subtask list in future similar tasks so the allowed touch-set matches reality — prevents false alarms on re-review. No code change needed here. **Acknowledged; no code change required.**
