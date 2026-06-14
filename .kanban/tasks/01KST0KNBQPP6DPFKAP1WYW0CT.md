---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd380
title: Fix window-layer focus drop (board/toolbar clicks + keys don't focus)
---
RESOLVED. Root cause: `<FocusScope>` (apps/kanban-app/ui/src/components/focus-scope.tsx) drove the visible `<FocusIndicator>` and `data-focused` off the transient `useFocusClaim` useState, which resets to false on every remount and only updates on kernel focus-changed *transitions*. The persistent focus truth (entity-focus store via `useOptionalIsDirectFocus(fq)`) was computed but used only for scrollIntoView. The board virtualizes/re-renders (board-data-sync) so cards remount constantly → transient state wiped → indicator gone; the kernel no-ops the re-focus (already focused) so it's never restored. The inspector never virtualizes, so its transient state stuck → "inspector focuses, board/toolbars don't." This is the divergent/duplicative path the user flagged.

Fix (focus-scope.tsx): the visible signal is now `effectiveFocused = focused || isFocused` (union of transient claim + persistent store), passed into SpatialFocusScopeBody as a new `isFocused` prop; drives both `data-focused` and `<FocusIndicator>`. Survives remount.

Why tests missed it (now closed): both JS test kernels accepted spatial_focus unconditionally and hand-emitted focus-changed, masking both the kernel's already-focused no-op and the remount wipe.
- Made the umbrella e2e harness faithful (apps/kanban-app/ui/src/test/spatial-shadow-registry.ts): tracks pushed layers; spatial_focus now drops when snapshot absent / layer_fq not pushed / fq not in snapshot, and no-ops when already focused — mirroring SpatialState::focus. This reproduced the bug (Family 1 card-click failed) before the fix.
- Added a `tracing::warn!` to the previously-silent `registry.layer(&snapshot.layer_fq)?` drop in crates/swissarmyhammer-focus/src/state.rs so a layer/registry desync is observable in `just logs`.

Verification (all green):
- cargo test -p swissarmyhammer-focus --lib: 60/60
- e2e umbrella (faithful harness): 29/29
- focus component suite (focus-on-click regression, focus-indicator, focus-scope.scroll-transition, inspector.repeat-open-focus [strict], entity-card.spatial): 42/42
- spatial/nav/inspector sweep (board-view.spatial, spatial-nav-jump-to, perspective-tab focus-indicator, nav-focus.command, inspector.close-restores-focus): 26/26
- tsc --noEmit: clean

Files changed:
- apps/kanban-app/ui/src/components/focus-scope.tsx (fix)
- apps/kanban-app/ui/src/test/spatial-shadow-registry.ts (faithful harness)
- crates/swissarmyhammer-focus/src/state.rs (observability log)

#bug #focus #spatial-nav