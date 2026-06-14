---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa480
project: ui-command-cleanup
title: Fix 2 pre-existing failures in grid-view.cursor-ring.test.tsx (cursor ring never renders in browser-mode tests)
---
## What
2 pre-existing test failures in `apps/kanban-app/ui/src/components/grid-view.cursor-ring.test.tsx` have no tracking card. Surfaced (not introduced) during review of Card C (`01KTED6YMERJHTS7QDSTV5MZYG`).

## Exact failing tests
1. `GridView -- cursor-ring suppression outside ui:grid > renders exactly one [data-cell-cursor] when focus is on a grid_cell moniker`
2. `GridView -- click-to-cursor regression (spatial path) > clicking a cell sets entity-focus and lights the cursor ring on that cell`

## Error output (both identical shape)
```
AssertionError: expected +0 to be 1 // Object.is equality
- Expected: 1
+ Received: 0
```
Zero `[data-cell-cursor]` cells render — the cursor ring never lights.

## Resolution (2026-06-12, HEAD af64be18c)
Both failures reproduced at HEAD. Root cause: the test's bespoke invoke mock stubbed the LEGACY Tauri focus commands (`spatial_focus` / `spatial_clear_focus` / `spatial_register_scope`) and emitted its own synthetic `focus-changed`. The focus kernel wire has since migrated to the in-process MCP transport — `focus-mcp.ts::setFocus` → `invoke("command_tool_call", { tool: "focus", op: "set focus", params: { fq, snapshot, window } })`, with focus claims routed through the `nav.focus` webview-bus command. None of the legacy stubs ever fired, so no `focus-changed` reached the `EntityFocusProvider` bridge and the entity-focus store (which `gridCellCursor` derives the ring from) never updated. Test bug, not a production bug — the production path is covered by `grid-view.spatial-nav.test.tsx` (passing).

Fix: rewrote the test scaffolding onto the shared kernel simulator `@/test/spatial-shadow-registry` (`setupSpatialHarness`), which models the current MCP wire including the kernel's snapshot-validated commit/drop conditions. Focus targets are now resolved to their real registered FQMs via `getRegisteredFqBySegment`; a real `<FocusScope moniker="ui:navbar">` sibling is mounted so out-of-grid focus is a genuinely registered target. The two formerly-vacuous suppression tests (`0 rings`) now first light the ring on a cell and assert the 1 → 0 transition. No production code changed; no shared helper changed. Verified: 5/5 tests pass, `npx tsc --noEmit` exit 0.

## Proof pre-existing
Run captured on HEAD `7c5015141764423b008b51d6c9d898d603b32288` BEFORE any Card C review-fix changes (2026-06-11): identical 2-failure set (`expected +0 to be 1` at lines 396 and 516). The Card C review also reproduced the identical failure set at the same HEAD in a clean worktree.

## Likely related
Card `01KTS1C4EX8W6GZYPAYB1T431K` describes the same symptom family (synthetic `focus-changed` emission not reaching the entity-focus store in browser mode) but enumerates only `focus-scope.test.tsx` (9) + `attachment-display.test.tsx` (1). Same root-cause family: legacy `spatial_*` invoke stubs vs the MCP focus wire — the shared-harness migration here is the reference fix.