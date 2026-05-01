---
assignees:
- claude-code
depends_on:
- 01KNHP391SXAQ5H2YXEK2MYJD1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffa180
title: 'WARNING: nav-bar.tsx constructs board moniker by hand for inspect button'
---
**File:** `kanban-app/ui/src/components/nav-bar.tsx` — inspect button onClick\n\n**What:** The board inspect button dispatches:\n```ts\ndispatchInspect({ target: moniker(\"board\", \"board\") })\n```\ninstead of using `board.board.moniker`.\n\n**Why:** The board entity is available via `useBoardData()` as `board.board`. Once the `entityFromBag` root-cause fix lands, `board.board.moniker` will be available. Using the backend moniker is the correct pattern going forward.\n\n**Suggestion:** Replace with `dispatchInspect({ target: board.board.moniker })`.\n\n**Verification:** Click the (i) inspect button in the nav bar, confirm the inspector opens for the board entity. #review-finding