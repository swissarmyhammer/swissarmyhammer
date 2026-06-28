---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd580
title: Up/down card nav lands on an inner scope of the target card, not the card
---
RESOLVED (kernel) + zone-skip enhancement. Symptom: Up/Down on a board card jumped into a field in the middle of the card above instead of the card.

Fix 1 (commit 2a05bc4ca): geometric_pick drops a candidate that descends from another candidate which is a SIBLING of the focused scope (same parent_zone); skips ancestors of focused. Card above is a sibling → its fields dropped, card wins. Cross-column still lands on the next card by beam score. Within-card field nav unaffected.

Fix 2 / addressed the flagged edge (commit 6744a834e): added SnapshotScope.focusable (mirrors <FocusScope showFocus>, serde-default true; React buildSnapshot sets it from the registry entry). geometric_pick skips non-focusable scopes as cardinal candidates, so a move passes through a structural zone (board well, perspective bar — showFocus=false) and lands on the focusable child beneath instead of the indicator-less zone. NOTE: this zone-landing was pre-existing (the zone was already the lowest-beam-score candidate), not a regression from Fix 1.

Tests: navigate.rs unit tests for cards+nested fields (up/down→card not field; within-card→fields; cross-column→card not title; down-from-navbar→card not board well). focus-crate 65 lib + all integration green; tsc clean; 80 focus/nav UI tests pass; kanban-app compiles.

Commits on plugin: 2a05bc4ca, 6744a834e (pushed). Files: navigate.rs, snapshot.rs (+focusable field), state.rs, registry.rs, focus integration tests, commands.rs, types/spatial.ts, layer-scope-registry-context.tsx, focus-scope.tsx.

Needs user verification in app: up/down card↔card; cross-column left/right→card; inside a card up/down→fields; arrow from nav bar → a card (not the invisible board well).

Follow-up still open: 01KST708RZ (reconcile diverged JS nav port so UI tests actually validate the kernel).

#bug #focus #spatial-nav #navigation