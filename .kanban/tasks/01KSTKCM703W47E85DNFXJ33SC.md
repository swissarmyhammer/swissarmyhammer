---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffd880
title: 'Spatial nav: tier-locked cardinal nav (cards are nav unit; fields via drill-in)'
---
User-reported: arrow keys on the board land "in the middle of the card above" — i.e. on a card's inner FIELD, not the card. Root cause (confirmed): click and jump (`s`) land focus on a card's inner field scope (the field FocusScope stopPropagation wins; jump enumerates fields), and the purely-geometric kernel then moves field→field into adjacent cards. Card→card nav already works WHEN focus is on a card.

User decision: option 1 (cards are the nav unit; Enter drills into fields) with a HARD constraint: NO SPECIAL CASES — must be one general rule, not a board/card-specific hack.

General model to implement (uniform across all layers):
- A focusable scope nested inside another focusable scope (field inside card) is reachable ONLY via drill-in (Enter), never via arrow/click/jump.
- Cardinal arrow nav operates among scopes sharing the focused scope's NEAREST FOCUSABLE ANCESTOR ("tier"). This preserves cross-area nav (composer↔board are both top-tier) while excluding cross-tier dives.
- Structural containers (columns, board well, panels, view-area) are non-focusable zones → cards are top-tier → arrows move card↔card across columns.
- Click / jump land on the top-tier (shallowest) focusable, i.e. the card, not a nested field.

Work items:
1. Kernel navigate.rs: tier-lock geometric_pick by nearest-focusable-ancestor; simplify the now-subsumed ancestor/tie-break logic; update module docs; rewrite the cross_parent_zone test to the tier model; add card/field tier tests.
2. React: columns non-focusable; ensure board well/panels/view-area are zones; nested focusables excluded from click & jump (drill-in only); click lands on the card.
3. JS test kernel (spatial-shadow-registry.ts) mirror tier-lock; update/extend vitest spatial tests.
4. Run cargo + vitest + tsc; verify no nav regressions.

Companion to the committed multi-window fix (a254c5cc7).