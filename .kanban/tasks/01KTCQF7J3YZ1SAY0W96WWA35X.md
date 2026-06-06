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
Find what CHANGED that stopped these buttons (and the nav-bar (i) inspect, #bug 01KTCRQGEXVKG0BZTF6KGE3Y35) from being jump targets, and restore the prior working behavior — without introducing a new flag. Investigate the jump-target enumeration (`jump-to-overlay.tsx::useJumpTargets` / `isTopTierFocusable`) and the focus-scope/rect registration history; determine why these focusable leaves used to be enumerated and no longer are. (Note: there was also a real `startsWith of undefined` crash in the jump activation path at jump-to-overlay.tsx ~line 493 worth fixing, but NOT via a jumpable flag.) TDD, RED first.