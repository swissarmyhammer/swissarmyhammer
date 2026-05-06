---
assignees:
- wballard
depends_on:
- 01KQW6H3397154YJWDPD6TDYZ3
position_column: todo
position_ordinal: df80
project: spatial-nav
title: Fix 16 spatial-nav test files broken by production-shape drift (board-view, perspective-tab-bar, etc.)
---
## Context

These tests pre-existed step 11's IPC cutover and were already failing at HEAD before that work. They reference production scope shapes that have drifted in earlier refactors:

- `ui:board` chrome zone — production `<BoardView>` no longer mounts a chrome `<FocusScope moniker={asSegment("ui:board")}>`. The board entity zone (`board:<id>`) directly wraps the columns. Tests still grep for `data-segment="ui:board"` and `<FocusScope moniker={asSegment("ui:board")}>` literals.
- `perspective_tab.name:{id}` inner leaf — collapsed into a single `perspective_tab:{id}` scope when the perspective-tab Pressable migration landed (commit `ce3a6ee60`). Tests still expect the inner `.name` leaf.

## Files

- src/components/board-view.cross-column-nav.spatial.test.tsx
- src/components/board-view.guards.node.test.ts
- src/components/board-view.spatial-nav.test.tsx
- src/components/board-view.spatial.test.tsx
- src/components/focus-on-click.regression.spatial.test.tsx
- src/components/grid-view.keyboard-nav.spatial.test.tsx
- src/components/perspective-bar.spatial.test.tsx
- src/components/perspective-spatial-nav.guards.node.test.ts
- src/components/perspective-tab-bar.context-menu.test.tsx
- src/components/perspective-tab-bar.delete-undo.test.tsx
- src/components/perspective-tab-bar.external-clear.test.tsx
- src/components/perspective-tab-bar.focus-indicator.browser.test.tsx
- src/components/perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx
- src/components/perspective-tab-bar.spatial-nav.test.tsx
- src/components/perspective-tab-bar.test.tsx
- src/spatial-nav-soak.spatial.test.tsx

## What

Update each test's expected scope shapes (segments, parent zones, DOM probes) to match the current production scope graph. Many are simple s/`ui:board`/`board:<id>`/g + parent zone updates; some need adjustment of the DOM probe paths.

## Acceptance Criteria
- `npx vitest run` from `kanban-app/ui` shows 0 failed test files.
- No regressions to currently-passing tests.

## Tests
- The fix IS the test suite. Each updated file should pass after the shape-drift fixes.

## Out of Scope
- IPC cutover work (already complete in 01KQW6H3397154YJWDPD6TDYZ3).
- Production scope graph changes — only adjust the test expectations.