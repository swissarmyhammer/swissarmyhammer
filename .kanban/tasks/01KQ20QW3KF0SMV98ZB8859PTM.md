---
assignees:
- claude-code
depends_on:
- 01KNQXW7HHHB8HW76K3PXH3G34
- 01KQ5PP55SAAVJ0V3HDJ1DGNBY
position_column: doing
position_ordinal: '8380'
project: spatial-nav
title: 'Toolbar: wrap action groups as zones, strip legacy keyboard nav'
---
## STATUS: REOPENED 2026-04-26 — same scope as NavBar card; rolling into that work

This card is rolled into the NavBar card `01KQ20Q2PNNR9VMES60QQSVXTS` since the inventory found that `nav-bar.tsx` is the only toolbar-style cluster in the app shell. All remaining work — verify visible focus on each nav bar leaf — is being tracked there.

## Outcome

When the NavBar card lands its visible-focus integration tests, this card moves to `done` automatically. No additional action required here.

---

(Original description and prior implementation notes preserved below for reference.)

## (Prior) Inventory result (2026-04-26)

The only toolbar-style cluster in the app shell lives in `nav-bar.tsx`; the wrapping landed under the NavBar sibling card. No `*-toolbar.tsx` / `*-actions.tsx` files exist. `app-shell.tsx` renders no buttons. The NavBar wrap (zone + 3 leaves) satisfies this card's structural acceptance criteria — but the visible-focus verification is now blocked behind the same fix as the NavBar card.