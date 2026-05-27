---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa680
title: 'Jump-To: skip off-viewport/occluded scopes so labels don''t collide under the AI panel'
---
## What

When the AI chat panel is open and the user triggers Jump-To, jump-label pills for board columns/cards collide with the panel's pills — pills stack at the same screen position and become unreadable.

### Corrected root cause (measured against the real `<App/>` in Chromium)

The original hypothesis (panel overlaps the board) was **wrong**. Measured: the AI panel is already a true non-overlapping right dock — panel left edge = board scroll-viewport right edge (they abut); the panel pushes the board. No layout bug.

The real cause is in **Jump-To** (`apps/kanban-app/ui/src/components/jump-to-overlay.tsx`). `useJumpTargets` enumerated every `<FocusScope>` in the topmost layer and filtered only `rect.width > 0 && rect.height > 0`. On a board wide enough to scroll horizontally, columns scrolled **off-screen** still have a geometric `getBoundingClientRect()` whose `left` lands under the panel (`overflow` clips the *paint*, not the rect). Jump-To painted a pill there → it stacked on the panel's pills. No viewport/occlusion check.

### Fix (done)

`useJumpTargets` now drops scopes whose pill-anchor is not actually visible, via `isScopeAnchorVisible(fq, rect)`: an `elementFromPoint` hit-test at the exact pill-anchor point keeps a scope only when the topmost painted element there is the scope's own `data-moniker` host (or a descendant) — selector built with `CSS.escape`. One panel-agnostic check covers all three invisibility cases — off-viewport, scrolled out of a clipping ancestor, occluded by a higher surface (the panel). The hit-test runs in the mount effect while the overlay body still renders `null`, so the `z-[80]` backdrop is not yet in the DOM. No change to Jump-To code generation, matching, dismiss, or pill rendering (only extracted the shared `PILL_ANCHOR_OFFSET`); no layout change.

## Acceptance Criteria

- [x] With the AI panel open and a board scrolled so a column/card is off-screen/occluded, that scope gets NO jump pill.
- [x] Visible board scopes still get pills; panel scopes still get pills; jumping to a visible target still works.
- [x] Triggering Jump-To with the panel open paints no two pills whose rects overlap (no board pill under a panel pill).
- [x] No regression: a board that fits (no overflow) labels every scope as before; Jump-To matching/dismiss/flash unchanged.

## Tests

- [x] `jump-to-overlay.occlusion.spatial.test.tsx` (new): full `<App/>` + open AI panel; occluded board scopes get no pill, painted board pills equal exactly the ground-truth-visible board scopes, panel scopes still labelled, no two pill rects overlap. Fail-before / pass-after confirmed.
- [x] Added `src/test/stub-scope-geometry.ts` and updated existing jump-to tests to the visible-only contract.
- [x] `ai-panel-dock.spatial.test.tsx` kept green (dock contract guard).
- [x] Full UI suite green: 256 files, 2438 tests; `tsc --noEmit` clean.

## Workflow

- `/tdd` — failing occlusion test first, then the `useJumpTargets` visibility filter.

## Review Findings (2026-05-23 — task-mode, reviewer subagent)

0 blockers, 0 warnings, 2 nits — both resolved.

### Nits
- [x] `jump-to-overlay.tsx` `isScopeAnchorVisible` — `querySelector([data-moniker="${fq}"])` could throw a selector `SyntaxError` (aborting the whole filter) if an FQM ever contained a `"`/`\`. Hardened with `CSS.escape(fq)`.
- [x] `stub-scope-geometry.ts` — documented that the stub's insertion-order `elementFromPoint` tie-break is only valid for non-overlapping fixtures; overlapping occlusion cases should use real layout.

Re-verified after fixes: full UI suite green (256 files, 2438 tests; tsc clean).

#bug #nav-jump