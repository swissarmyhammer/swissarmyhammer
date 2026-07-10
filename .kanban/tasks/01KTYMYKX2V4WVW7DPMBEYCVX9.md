---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvzf37phqc1vw83v4tw9myye
  text: 'Picked up. Verified current code in perspective-context.tsx matches the task description: useAutoSelectActivePerspective builds `matching` kind-only (`p.view === viewKind`); activePerspective memo fallback `perspectives.find(...) ?? perspectives[0] ?? null` is not even kind-filtered. Shared predicate perspectiveVisibleInView(p, activeViewId, viewKind) lives in types/kanban.ts and is already used by useAutoCreateDefaultPerspective (which is threaded `activeViewId` via PerspectiveProvider). Starting TDD: writing failing vitest cases first.'
  timestamp: 2026-06-25T13:21:48.241172+00:00
- actor: claude-code
  id: 01kvzf7d8rkfp3hrwand9r2wjj
  text: |-
    Implemented and GREEN. Changes (apps/kanban-app/ui):
    - src/lib/perspective-context.tsx: useAutoSelectActivePerspective now builds `matching` via perspectiveVisibleInView(p, activeViewId, viewKind) instead of `p.view === viewKind`; threaded `activeViewId` param + dep (mirrors useAutoCreateDefaultPerspective; call site passes the same `activeViewId = activeView?.id` source). activePerspective memo now filters perspectives to the VISIBLE set first, then `find(id) ?? visible[0] ?? null` (was unfiltered find ?? perspectives[0]); added activeViewId+viewKind to deps.
    - src/lib/perspective-context.test.tsx: 3 new TDD cases (sibling-pinned not auto-selected; visible wins; sibling-pinned stored id rejected; activePerspective fallback skips sibling-pinned).

    TDD RED evidence: before the production change, the 3 new tests run against the original kind-only code gave "3 failed | 2 passed" (browser/chromium project). After the fix: 31 passed (both happy-dom unit and browser projects), stable across 3 reruns.

    Verification: npx tsc --noEmit clean (exit 0); perspective-context.test.tsx 31/31; perspective-tab-bar.test.tsx 22/22; view-id-scoping + perspective-container + perspectives-container + grid-view.perspective-filter 20/20.
  timestamp: 2026-06-25T13:24:05.016658+00:00
- actor: wballard
  id: 01kvzfv8g2dgs5a862jq51e3np
  text: |-
    Review resolution — IN-SCOPE CLEAN, 0 blockers. Reviewer independently verified all 5 load-bearing points: shared `perspectiveVisibleInView` predicate REUSED (single source of truth shared with the tab bar + useAutoCreateDefaultPerspective, not duplicated); all 3 repair paths in useAutoSelectActivePerspective + the activePerspective memo's find/[0] fallback constrained to the visible set; dep arrays correctly extended (activeViewId on the effect, activeViewId+viewKind on the memo); activeViewId threading mirrors the useAutoCreateDefaultPerspective precedent (both callers updated atomically, no signature break); TDD genuine (reviewer reverted predicate → 2-3 tests fail, restored → green).

    All 6 engine findings (active_perspective_id snake_case ×2 [backend field name], PerspectiveProvider length, missing PerspectiveProviderProps interface, missing JSDoc ×2) are PRE-EXISTING whole-file noise NOT touching the changed lines — out of scope, no action.

    NOTE: during the reviewer's RED check it accidentally `git checkout -- perspective-context.tsx` (discarding the uncommitted fix) then restored from the captured diff. I INDEPENDENTLY RE-VERIFIED the working tree afterward: both new call sites present (perspective-context.tsx lines 227 + 374), `git diff --stat HEAD` shows both files changed (+35 prod / +85 test), `npx tsc --noEmit` clean, and perspective-context.test.tsx 31 + perspective-tab-bar.test.tsx 22 = 53/53 pass. The fix is fully intact byte-for-byte. Moving to done.
  timestamp: 2026-06-25T13:34:55.490241+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffea80
title: Auto-select / activePerspective fallback are kind-only and can select a perspective invisible in the active view's tab bar — route both through perspectiveVisibleInView
---
## What

Pre-existing gap surfaced during review of 01KTY6T1GPY94VYWANE9X41SKJ (iteration-4 nit): perspective *selection* still matches kind-only while perspective *visibility* (tab bar + auto-create guard) is view_id-first. The selected/active perspective can therefore be one pinned to a sibling view — i.e. a tab the user cannot see in the active view's bar.

## Exact locations (`apps/kanban-app/ui/src/lib/perspective-context.tsx`)

1. `useAutoSelectActivePerspective` (hook defined around line 208): line 218 — `const matching = perspectives.filter((p) => p.view === viewKind);`. All three repair paths (invalid/empty stored id → switch to `matching[0]`; valid-id stale-filter redispatch; validity check `matching.some((p) => p.id === active_perspective_id)`) operate on this kind-only set, so `perspective.switch` can be dispatched for a perspective pinned (`view_id`) to a different view of the same kind.
2. `activePerspective` memo in `PerspectiveProvider` (lines 354–360): the fallback `perspectives.find((p) => p.id === active_perspective_id) ?? perspectives[0] ?? null` isn't even kind-filtered — `perspectives[0]` can be any kind AND any pinned view.

## Symptom

A perspective that is invisible in the active view's tab bar (pinned via `view_id` to a sibling view, filtered out by the bar's `filteredPerspectives` memo in `perspective-tab-bar.tsx`) can be auto-selected as active or fall back as `activePerspective`. The board then filters/groups by a perspective the user has no visible tab for — no highlighted tab, no way to see or switch off it from the bar.

## Fix direction

Reuse the shared predicate `perspectiveVisibleInView(p, activeViewId, viewKind)` exported from `apps/kanban-app/ui/src/types/kanban.ts` (line 98) — the single source of truth already shared by the tab bar's filter (`perspective-tab-bar.tsx`) and `useAutoCreateDefaultPerspective`:

- In `useAutoSelectActivePerspective`, build `matching` with `perspectiveVisibleInView` instead of `p.view === viewKind` (requires threading `activeViewId` into the hook, same as was done for `useAutoCreateDefaultPerspective` in iteration 4).
- In the `activePerspective` memo, constrain the `perspectives[0]` fallback (and arguably the `find`) to visible perspectives via the same predicate.

## Tests

- Vitest in `apps/kanban-app/ui/src/lib/perspective-context.test.tsx` (existing mock/harness patterns): a perspective pinned to a sibling view must NOT be auto-selected nor returned as the `activePerspective` fallback; a visible one must win. TDD red-check: revert the predicate to kind-only and confirm the new tests fail.

## Constraints

- Scoped vitest + `npx tsc --noEmit` in apps/kanban-app/ui only; no workspace builds. #ui

## Review Findings (2026-06-25 07:34)

**In-scope verdict: CLEAN.** The load-bearing change is correct on all 5 review-focus points and the new tests are genuine (RED→GREEN verified by the reviewer — see below). Every engine finding below is **pre-existing whole-file noise** that does NOT touch the changed lines; none are introduced or worsened by this diff. They are recorded for traceability, not as blockers for this task.

### In-scope assessment (reviewer-verified, no action needed)
- [x] Shared-predicate single-source-of-truth: `useAutoSelectActivePerspective` and the `activePerspective` memo now both filter via `perspectiveVisibleInView(p, activeViewId, viewKind)` imported from `src/types/kanban.ts` — the SAME predicate the tab bar's `filteredPerspectives` and `useAutoCreateDefaultPerspective` use. Reused, not duplicated; selection and visibility can no longer diverge.
- [x] All three repair paths in `useAutoSelectActivePerspective` (invalid/empty id → `matching[0]`; stale-filter redispatch; `matching.some(...)` validity) now operate on the visible set. `activePerspective` memo's `find` AND `[0]` fallback both constrained to `visible`.
- [x] Dependency arrays correct: `useEffect` deps gained `activeViewId` (viewKind already present); `useMemo` deps gained `activeViewId, viewKind`. No stale-selection risk.
- [x] `activeViewId` threading mirrors the `useAutoCreateDefaultPerspective` precedent (param position before `viewKind`; call site passes `activeView?.id`). Both callers updated atomically — no signature break.
- [x] TDD genuineness verified by reviewer: reverting `useAutoSelectActivePerspective` predicate to kind-only → 2 new tests fail; additionally reverting the `activePerspective` memo to unfiltered → all 3 new tests fail. Restored production code: `npx tsc --noEmit` exit 0; `perspective-context.test.tsx` 31/31 + `perspective-tab-bar.test.tsx` 22/22 = 53/53 GREEN. No regression.

### Out-of-scope / pre-existing (engine fan-out — NOT introduced by this diff)
- [ ] `perspective-context.tsx` — Parameter `active_perspective_id` uses snake_case rather than camelCase. PRE-EXISTING: name mirrors the backend `uiState.windows[...].active_perspective_id` field and predates HEAD; this diff neither adds nor renames it.
- [ ] `perspective-context.tsx` — Local variable `active_perspective_id` uses snake_case. PRE-EXISTING, same backend-field origin as above.
- [ ] `perspective-context.tsx` — `PerspectiveProvider` exceeds the ~50-line threshold; suggest extracting hook composition into `usePerspectiveRepair`/`usePerspectiveSelection`. PRE-EXISTING: the component was already this size at HEAD; this diff added ~7 net lines of memo body.
- [ ] `perspective-context.tsx` — `PerspectiveProvider` lacks a named `interface PerspectiveProviderProps`. PRE-EXISTING: inline `{ children }: { children: ReactNode }` predates HEAD.
- [ ] `perspective-context.tsx` — `PerspectivesContextValue` interface lacks JSDoc. PRE-EXISTING, untouched by this diff.
- [ ] `perspective-context.tsx` — `PerspectiveProvider` lacks a JSDoc usage comment. PRE-EXISTING, untouched by this diff.