---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb680
title: 'WARNING: DataTable header click — Shift+click and plain click branches are identical dead code'
---
**File**: kanban-app/ui/src/components/data-table.tsx (handleHeaderClick, around the header click handler)\n\n**What**: The `handleHeaderClick` function checks `e.shiftKey` but both branches execute identical code — the same `dispatchSortToggle` call with identical args. The comment says \"Shift+click appends to multi-sort via set; plain click toggles\" but the behavior is the same.\n\n**Why**: The intended behavior (Shift+click adds to multi-sort, plain click replaces sort) is not implemented. Users cannot distinguish between the two actions.\n\n**Suggestion**: Pass a `mode` argument in the dispatch args to differentiate (e.g., `mode: 'toggle'` for plain click, `mode: 'append'` for Shift+click), or remove the dead branch.\n\n**Subtasks**:\n- [ ] Differentiate Shift+click (append) from plain click (replace) in the dispatch args\n- [ ] Add test that Shift+click sends different args than plain click\n- [ ] Verify fix by running tests #review-finding