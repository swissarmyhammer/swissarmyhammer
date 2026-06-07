---
assignees:
- claude-code
position_column: todo
position_ordinal: d080
title: Jump (s) cannot target the (i) inspect and (x) close buttons
---
REOPENED 2026-06-06 — prior fix was WRONG and discarded.

## OWNER CORRECTION (authoritative)
Do NOT add a `jumpable` flag. We did NOT need one before and jump WORKED. So the (i) inspect / (x) close buttons not being jump targets is a REGRESSION to find and restore — not a missing opt-in to invent. The discarded approach added a `jumpable` prop threaded through Pressable/FocusScope/layer-scope-registry/spatial-focus-context and relaxed the `useJumpTargets` top-tier filter to honor it. All reverted to HEAD.

## Next step
Find what CHANGED that stopped these buttons (and the nav-bar (i) inspect) from being jump targets, and restore the prior working behavior — without introducing a new flag. Investigate the jump-target enumeration (`jump-to-overlay.tsx::useJumpTargets` / `isTopTierFocusable`) and the focus-scope/rect registration history; determine why these focusable leaves used to be enumerated and no longer are. (Note: there was also a real `startsWith of undefined` crash in the jump activation path at jump-to-overlay.tsx ~line 493 worth fixing, but NOT via a jumpable flag.) TDD, RED first.

## Learnings folded in from duplicate (was #bug 01KTCRQGEXVKG0BZTF6KGE3Y35, now deleted — this card subsumes the nav-bar (i) inspect case)
- The nav-bar (i) inspect button registers as a peer focus leaf `<Pressable moniker="ui:navbar.inspect">` at `apps/kanban-app/ui/src/components/nav-bar.tsx:107`, sibling to `ui:left-nav` / `ui:perspective-bar` under `<FocusLayer name="window">`. It only renders when a board is open.
- Bisect repro for the enumeration regression: trigger jump and check whether `ui:navbar.search` (the adjacent search button) gets a sneak code while `ui:navbar.inspect` does NOT.
  - Both missing → the navbar leaves aren't enumerated into the jump layer at all (layer-set / enumeration scope problem).
  - Only inspect missing → per-leaf rect/filter problem.
- Three hypotheses for why these leaves dropped out of `useJumpTargets`: (a) the leaf publishes a zero/absent rect to the kernel, (b) it's filtered out by the top-tier filter (`isTopTierFocusable`), (c) the navbar leaves' layer isn't in the set `useJumpTargets` reads.
- Suggested tests: `apps/kanban-app/ui/src/spatial-nav-jump-to.spatial.test.tsx` and `apps/kanban-app/ui/src/components/jump-to-overlay.*.test.tsx` — assert `ui:navbar.inspect` (and the (x) close) are enumerated jump targets when a board is open.